//! Serial ordered event uploader with batching, retry, and backpressure.
//! 2.1.88 reference: `src/cli/transports/SerialBatchEventUploader.ts`.
//!
//! - `enqueue` adds events to a pending buffer
//! - At most 1 POST in flight at a time
//! - Drains up to `max_batch_size` items per POST
//! - New events accumulate while in flight
//! - On failure: exponential backoff (clamped), retries indefinitely until
//!   success or `close` — unless `max_consecutive_failures` is set, in which
//!   case the failing batch is dropped and drain advances
//! - `flush` waits until pending is empty and kicks drain if needed
//! - Backpressure: `enqueue` waits when `max_queue_size` is reached

use serde_json::Value;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::Semaphore;

/// `send` failure mode: `retry_after_ms` (e.g. from a 429 Retry-After header)
/// overrides exponential backoff, clamped to `max_delay_ms`.
#[derive(Debug, Clone)]
pub enum UploaderError {
    Retryable { retry_after_ms: Option<u64> },
}

impl std::fmt::Display for UploaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Retryable { retry_after_ms } => {
                write!(f, "upload retry required: {retry_after_ms:?}")
            }
        }
    }
}
impl std::error::Error for UploaderError {}

pub type UploaderSend<T> = Arc<
    dyn Fn(
            Vec<T>,
        )
            -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), UploaderError>> + Send>>
        + Send
        + Sync,
>;

pub struct UploaderConfig<T> {
    /// Max items per POST.
    pub max_batch_size: usize,
    /// Optional max serialized bytes per POST; the batch stops before the
    /// first item that would exceed the bound.
    pub max_batch_bytes: Option<usize>,
    /// Queue bound. `enqueue` waits when adding `events.len()` items would
    /// exceed this.
    pub max_queue_size: usize,
    /// Exponential backoff base delay.
    pub base_delay_ms: u64,
    /// Backoff clamp.
    pub max_delay_ms: u64,
    /// Uniform jitter added to every delay, in `[0, jitter_ms)`.
    pub jitter_ms: u64,
    /// Optional cap so a persistently-failing server cannot pin the drain
    /// loop for the process lifetime. `None` = indefinite retry.
    pub max_consecutive_failures: Option<u32>,
    /// Invoked when a batch is dropped due to `max_consecutive_failures`.
    pub on_batch_dropped: Option<Arc<dyn Fn(usize, u32) + Send + Sync>>,
    pub send: UploaderSend<T>,
}

impl<T> UploaderConfig<T> {
    pub fn new(send: UploaderSend<T>) -> Self {
        Self {
            max_batch_size: 100,
            max_batch_bytes: None,
            max_queue_size: 100_000,
            base_delay_ms: 500,
            max_delay_ms: 30_000,
            jitter_ms: 500,
            max_consecutive_failures: None,
            on_batch_dropped: None,
            send,
        }
    }
}

struct UploaderState<T> {
    pending: std::collections::VecDeque<T>,
    draining: bool,
    closed: bool,
    pending_at_close: usize,
    /// Waiters registered on the semaphore (backpressure or flush).
    waiters: usize,
}

impl<T: serde::Serialize> UploaderState<T> {
    /// Take the next batch, mirroring the source: the first item is always
    /// taken; byte-limited configs stop before the first item that would
    /// exceed the bound once the batch is non-empty.
    fn take_batch(&mut self, config: &UploaderConfig<T>) -> Vec<T> {
        let mut batch: Vec<T> = Vec::new();
        let mut bytes = 0usize;
        while let Some(item) = self.pending.front() {
            if batch.len() >= config.max_batch_size {
                break;
            }
            if let Some(limit) = config.max_batch_bytes {
                let item_bytes = item_bytes(item).unwrap_or(0);
                if !batch.is_empty() && bytes + item_bytes > limit {
                    break;
                }
                bytes += item_bytes;
            }
            batch.push(self.pending.pop_front().expect("front checked"));
        }
        batch
    }
}

/// Serialized size of an event; `None` falls back to zero like the source's
/// `jsonStringify`-throwing items being skipped from the byte budget.
fn item_bytes<T: serde::Serialize>(item: &T) -> Option<usize> {
    serde_json::to_vec(item).ok().map(|bytes| bytes.len())
}

struct Shared<T> {
    config: UploaderConfig<T>,
    state: Mutex<UploaderState<T>>,
    /// One permit released per waiting enqueue/flush continuation.
    wake: Arc<Semaphore>,
    closed: AtomicBool,
    dropped_batches: AtomicUsize,
}

pub struct SerialBatchEventUploader<T: serde::Serialize + Send + Clone + 'static> {
    shared: Arc<Shared<T>>,
}

impl<T: serde::Serialize + Send + Clone + 'static> SerialBatchEventUploader<T> {
    pub fn new(config: UploaderConfig<T>) -> Self {
        Self {
            shared: Arc::new(Shared {
                config,
                state: Mutex::new(UploaderState {
                    pending: std::collections::VecDeque::new(),
                    draining: false,
                    closed: false,
                    pending_at_close: 0,
                    waiters: 0,
                }),
                wake: Arc::new(Semaphore::new(0)),
                closed: AtomicBool::new(false),
                dropped_batches: AtomicUsize::new(0),
            }),
        }
    }

    pub fn pending_count(&self) -> usize {
        let state = self.shared.state.lock().expect("uploader state");
        if state.closed {
            state.pending_at_close
        } else {
            state.pending.len()
        }
    }

    pub fn dropped_batch_count(&self) -> usize {
        self.shared.dropped_batches.load(Ordering::SeqCst)
    }

    fn is_closed(&self) -> bool {
        self.shared.closed.load(Ordering::SeqCst)
    }

    /// Enqueue events, waiting on backpressure when the queue is full.
    /// Returns once events are buffered (drain runs in the background).
    pub async fn enqueue(&self, events: Vec<T>) {
        let mut remaining: std::collections::VecDeque<T> = events.into();
        while !remaining.is_empty() {
            if self.is_closed() {
                return;
            }
            let needs_wait = {
                let mut state = self.shared.state.lock().expect("uploader state");
                if state.closed {
                    return;
                }
                let capacity = self
                    .shared
                    .config
                    .max_queue_size
                    .saturating_sub(state.pending.len());
                if capacity > 0 {
                    let count = capacity.min(remaining.len());
                    state.pending.extend(remaining.drain(..count));
                    ensure_draining(&self.shared, &mut state);
                }
                if remaining.is_empty() {
                    false
                } else {
                    state.waiters += 1;
                    true
                }
            };
            if needs_wait && self.shared.wake.clone().acquire_owned().await.is_err() {
                return;
            }
        }
    }

    /// Wait until pending is empty (drain finished); kicks drain if needed.
    pub async fn flush(&self) {
        loop {
            if self.is_closed() {
                return;
            }
            {
                let mut state = self.shared.state.lock().expect("uploader state");
                if state.closed {
                    return;
                }
                if state.pending.is_empty() && !state.draining {
                    return;
                }
                if !state.draining {
                    state.draining = true;
                    spawn_drain(self.shared.clone());
                }
                state.waiters += 1;
            }
            match self.shared.wake.clone().acquire_owned().await {
                Ok(_permit) => continue,
                Err(_) => return,
            }
        }
    }

    /// Stop accepting new events and drop pending work. The in-flight batch,
    /// if any, finishes on its own.
    pub fn close(&self) {
        self.shared.closed.store(true, Ordering::SeqCst);
        let mut state = self.shared.state.lock().expect("uploader state");
        if state.closed {
            return;
        }
        state.closed = true;
        state.pending_at_close = state.pending.len();
        state.pending.clear();
        release_waiters(&self.shared, &mut state);
    }
}

async fn drain_loop<T: serde::Serialize + Send + Clone + 'static>(shared: Arc<Shared<T>>) {
    let mut failures: u32 = 0;
    loop {
        let batch = {
            let mut state = shared.state.lock().expect("uploader state");
            if state.closed {
                state.draining = false;
                release_waiters(&shared, &mut state);
                return;
            }
            let batch = state.take_batch(&shared.config);
            if batch.is_empty() {
                state.draining = false;
                release_waiters(&shared, &mut state);
                return;
            }
            batch
        };
        let batch_size = batch.len();
        match (shared.config.send)(batch.clone()).await {
            Ok(()) => {
                failures = 0;
                let mut state = shared.state.lock().expect("uploader state");
                if state.closed {
                    break;
                }
                release_waiters(&shared, &mut state);
                continue;
            }
            Err(UploaderError::Retryable { retry_after_ms }) => {
                failures += 1;
                // Put the failed batch back at the queue front so ordering
                // survives the retry (source: unshift(batch)).
                {
                    let mut state = shared.state.lock().expect("uploader state");
                    for item in batch.iter().rev() {
                        state.pending.push_front(item.clone());
                    }
                }
                if let Some(max) = shared.config.max_consecutive_failures {
                    if failures >= max {
                        {
                            let mut state = shared.state.lock().expect("uploader state");
                            for _ in 0..batch_size {
                                state.pending.pop_front();
                            }
                        }
                        shared.dropped_batches.fetch_add(1, Ordering::SeqCst);
                        if let Some(handler) = &shared.config.on_batch_dropped {
                            handler(batch_size, failures);
                        }
                        failures = 0;
                        let mut state = shared.state.lock().expect("uploader state");
                        if state.closed {
                            break;
                        }
                        release_waiters(&shared, &mut state);
                        continue;
                    }
                }
                // Backoff sleep is not interruptible — matching the source's
                // plain `setTimeout`; close() takes effect on the next loop
                // head via the closed check below.
                let delay = retry_delay(
                    failures,
                    retry_after_ms,
                    shared.config.base_delay_ms,
                    shared.config.max_delay_ms,
                    shared.config.jitter_ms,
                );
                tokio::time::sleep(delay).await;
                if shared.closed.load(Ordering::SeqCst) {
                    break;
                }
            }
        }
    }
    let mut state = shared.state.lock().expect("uploader state");
    state.draining = false;
    release_waiters(&shared, &mut state);
}

fn release_waiters<T>(shared: &Arc<Shared<T>>, state: &mut UploaderState<T>) {
    let waiters = state.waiters;
    state.waiters = 0;
    if waiters > 0 {
        shared.wake.add_permits(waiters);
    }
}

fn ensure_draining<T: serde::Serialize + Send + Clone + 'static>(
    shared: &Arc<Shared<T>>,
    state: &mut UploaderState<T>,
) {
    if !state.draining && !state.closed {
        state.draining = true;
        tokio::spawn(drain_loop(shared.clone()));
    }
}

fn spawn_drain<T: serde::Serialize + Send + Clone + 'static>(shared: Arc<Shared<T>>) {
    tokio::spawn(drain_loop(shared));
}

/// Mirror the source's delay math: Retry-After (clamped to
/// `[base_delay_ms, max_delay_ms]`) or exponential backoff
/// (`base * 2^(failures-1)` clamped), plus uniform jitter.
pub(super) fn retry_delay(
    failures: u32,
    retry_after_ms: Option<u64>,
    base_delay_ms: u64,
    max_delay_ms: u64,
    jitter_ms: u64,
) -> std::time::Duration {
    let jitter = rand_below(jitter_ms);
    let clamped = match retry_after_ms {
        Some(retry_after) => retry_after.max(base_delay_ms).min(max_delay_ms),
        None => base_delay_ms
            .saturating_mul(1u64 << failures.saturating_sub(1).min(16))
            .min(max_delay_ms),
    };
    std::time::Duration::from_millis(clamped.saturating_add(jitter))
}

fn rand_below(range: u64) -> u64 {
    if range == 0 {
        return 0;
    }
    let mut bytes = [0_u8; 8];
    if getrandom::getrandom(&mut bytes).is_ok() {
        u64::from_le_bytes(bytes) % range
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::AtomicUsize;

    fn config_with(
        send: UploaderSend<Value>,
        max_batch_size: usize,
        max_queue_size: usize,
    ) -> UploaderConfig<Value> {
        UploaderConfig {
            max_batch_size,
            max_batch_bytes: None,
            max_queue_size,
            base_delay_ms: 1,
            max_delay_ms: 4,
            jitter_ms: 0,
            max_consecutive_failures: None,
            on_batch_dropped: None,
            send,
        }
    }

    #[tokio::test]
    async fn batches_and_drains_in_order() {
        let posts = Arc::new(Mutex::new(Vec::<Vec<Value>>::new()));
        let sink = posts.clone();
        let uploader = SerialBatchEventUploader::new(config_with(
            Arc::new(move |batch| {
                let sink = sink.clone();
                Box::pin(async move {
                    sink.lock().unwrap().push(batch);
                    Ok(())
                })
            }),
            2,
            100,
        ));
        for index in 0..5 {
            uploader.enqueue(vec![json!({"i": index})]).await;
        }
        uploader.flush().await;
        let posts = posts.lock().unwrap();
        assert_eq!(posts.len(), 3);
        assert_eq!(posts[0].len(), 2);
        assert_eq!(posts[2][0], json!({"i": 4}));
    }

    #[tokio::test]
    async fn retries_then_succeeds_and_keeps_order() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let counter = attempts.clone();
        let received = Arc::new(Mutex::new(Vec::<Value>::new()));
        let sink = received.clone();
        let mut config = config_with(
            Arc::new(move |batch| {
                let counter = counter.clone();
                let sink = sink.clone();
                Box::pin(async move {
                    if counter.fetch_add(1, Ordering::SeqCst) < 2 {
                        // Failed batch must come back to the queue front.
                        for item in batch {
                            sink.lock().unwrap().push(json!({"failed": item}));
                        }
                        Err(UploaderError::Retryable {
                            retry_after_ms: None,
                        })
                    } else {
                        sink.lock().unwrap().extend(batch);
                        Ok(())
                    }
                })
            }),
            1,
            10,
        );
        config.base_delay_ms = 1;
        config.max_delay_ms = 2;
        config.jitter_ms = 0;
        let uploader = SerialBatchEventUploader::new(config);
        uploader.enqueue(vec![json!("x")]).await;
        uploader.flush().await;
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
        assert_eq!(uploader.pending_count(), 0);
        let received = received.lock().unwrap();
        assert_eq!(
            received.last(),
            Some(&json!("x")),
            "failed attempts requeue; final success delivers the item"
        );
    }

    #[tokio::test]
    async fn drops_batch_after_max_consecutive_failures() {
        let dropped = Arc::new(Mutex::new(Vec::<(usize, u32)>::new()));
        let sink = dropped.clone();
        let attempts = Arc::new(AtomicUsize::new(0));
        let counter = attempts.clone();
        let mut config = config_with(
            Arc::new(move |_batch| {
                let counter = counter.clone();
                Box::pin(async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    Err(UploaderError::Retryable {
                        retry_after_ms: None,
                    })
                })
            }),
            1,
            10,
        );
        config.base_delay_ms = 1;
        config.max_delay_ms = 2;
        config.max_consecutive_failures = Some(2);
        config.on_batch_dropped = Some(Arc::new(move |size, failures| {
            sink.lock().unwrap().push((size, failures));
        }));
        let uploader = SerialBatchEventUploader::new(config);
        uploader.enqueue(vec![json!("a"), json!("b")]).await;
        tokio::time::sleep(std::time::Duration::from_millis(80)).await;
        assert_eq!(uploader.dropped_batch_count(), 2);
        let dropped = dropped.lock().unwrap();
        assert_eq!(dropped.as_slice(), &[(1, 2), (1, 2)]);
        assert!(attempts.load(Ordering::SeqCst) >= 2);
    }

    #[tokio::test]
    async fn retry_after_overrides_backoff_within_clamp() {
        let mut config = config_with(Arc::new(|_batch| Box::pin(async { Ok(()) })), 1, 10);
        config.base_delay_ms = 100;
        config.max_delay_ms = 200;
        config.jitter_ms = 10;
        let delay = retry_delay(
            1,
            Some(5_000),
            config.base_delay_ms,
            config.max_delay_ms,
            config.jitter_ms,
        );
        assert!(delay.as_millis() >= 200, "retry-after clamped to max");
        let delay = retry_delay(
            3,
            None,
            config.base_delay_ms,
            config.max_delay_ms,
            config.jitter_ms,
        );
        assert!(delay.as_millis() <= 210);
    }

    #[tokio::test]
    async fn backpressure_waits_until_drain_advances() {
        let in_flight = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let gate = Arc::new(tokio::sync::Notify::new());
        let gate_for_send = gate.clone();
        let in_flight_for_send = in_flight.clone();
        let uploader = Arc::new(SerialBatchEventUploader::new(UploaderConfig {
            max_batch_size: 1,
            max_batch_bytes: None,
            max_queue_size: 2,
            base_delay_ms: 1,
            max_delay_ms: 2,
            jitter_ms: 0,
            max_consecutive_failures: None,
            on_batch_dropped: None,
            send: Arc::new(move |_batch| {
                let gate = gate_for_send.clone();
                let in_flight = in_flight_for_send.clone();
                Box::pin(async move {
                    in_flight.fetch_add(1, Ordering::SeqCst);
                    gate.notified().await;
                    in_flight.fetch_sub(1, Ordering::SeqCst);
                    Ok(())
                })
            }),
        }));
        let enqueue_task = {
            let uploader = Arc::clone(&uploader);
            async move {
                uploader.enqueue(vec![json!(1), json!(2), json!(3)]).await;
            }
        };
        let task = tokio::spawn(enqueue_task);
        // Queue holds 2; the third enqueue must be waiting on backpressure.
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        assert_eq!(in_flight.load(Ordering::SeqCst), 1);
        for _ in 0..3 {
            gate.notify_one();
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        tokio::time::timeout(std::time::Duration::from_secs(2), task)
            .await
            .expect("enqueue finishes after drain releases backpressure")
            .unwrap();
        uploader.flush().await;
        assert_eq!(uploader.pending_count(), 0);
    }

    #[tokio::test]
    async fn close_stops_enqueue_and_clears_pending() {
        let uploader = SerialBatchEventUploader::new(config_with(
            Arc::new(|_batch| Box::pin(async { Ok(()) })),
            1,
            10,
        ));
        uploader.close();
        uploader.enqueue(vec![json!("late")]).await;
        assert_eq!(uploader.pending_count(), 0);
        uploader.flush().await;
    }

    #[test]
    fn close_preserves_pending_depth_for_delivery_diagnostics() {
        let uploader = SerialBatchEventUploader::new(config_with(
            Arc::new(|_batch| Box::pin(async { Ok(()) })),
            1,
            10,
        ));
        uploader
            .shared
            .state
            .lock()
            .unwrap()
            .pending
            .push_back(json!({"uuid":"undelivered"}));
        uploader.close();
        assert_eq!(uploader.pending_count(), 1);
        assert!(uploader.shared.state.lock().unwrap().pending.is_empty());
        uploader.close();
        assert_eq!(uploader.pending_count(), 1);
    }

    #[test]
    fn byte_limited_batches_split() {
        let mut state = UploaderState::<Value> {
            pending: std::collections::VecDeque::new(),
            draining: false,
            closed: false,
            pending_at_close: 0,
            waiters: 0,
        };
        for index in 0..3 {
            state
                .pending
                .push_back(json!({"pad": "x".repeat(10), "i": index}));
        }
        let mut config = config_with(Arc::new(|_b| Box::pin(async { Ok(()) })), 10, 10);
        config.max_batch_bytes = Some(56);
        let batch = state.take_batch(&config);
        // Each JSON item is 26 bytes; two fit in 56, three do not.
        assert_eq!(batch.len(), 2);
    }

    #[tokio::test]
    async fn enqueue_after_drain_completes_restarts_loop() {
        let posts = Arc::new(AtomicUsize::new(0));
        let counter = posts.clone();
        let uploader = SerialBatchEventUploader::new(config_with(
            Arc::new(move |_batch| {
                let counter = counter.clone();
                Box::pin(async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                })
            }),
            1,
            10,
        ));
        uploader.enqueue(vec![json!(1)]).await;
        uploader.flush().await;
        // New work after the previous drain finished must still be delivered.
        uploader.enqueue(vec![json!(2)]).await;
        uploader.flush().await;
        assert_eq!(posts.load(Ordering::SeqCst), 2);
    }
}
