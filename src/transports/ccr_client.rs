//! CCR v2 worker client: epoch lifecycle, heartbeats, event upload.
//! 2.1.88 reference: `src/cli/transports/ccrClient.ts` (replacing the
//! provisional AST stub `generated_ports/p_03ce73c80da6a`).
//!
//! All endpoints are session-scoped paths under a self-hosted base URL; the
//! client never contacts original vendor endpoints. Writes reuse
//! `SerialBatchEventUploader` (client events / internal events / delivery
//! acknowledgements) and `WorkerStateUploader` for the coalesced worker PUT.

use anyhow::{anyhow, bail, Result};
use base64::Engine;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::serial_batch_event_uploader::{
    SerialBatchEventUploader, UploaderConfig, UploaderError, UploaderSend,
};
use super::sse::SseTransport;
use super::worker_state_uploader::{WorkerStateSend, WorkerStateUploader};

pub const STREAM_EVENT_FLUSH_INTERVAL_MS: u64 = 100;
pub const DEFAULT_HEARTBEAT_INTERVAL_MS: u64 = 20_000;
pub const DEFAULT_HEARTBEAT_JITTER_FRACTION: f64 = 0.1;
pub const AUTH_FAILURE_THRESHOLD: u32 = 10;
pub const CLIENT_EVENT_BASE_DELAY_MS: u64 = 500;
pub const CLIENT_EVENT_MAX_DELAY_MS: u64 = 30_000;
pub const CLIENT_EVENT_JITTER_MS: u64 = 500;
pub const CLIENT_EVENT_MAX_QUEUE: usize = 100_000;
pub const INTERNAL_EVENT_MAX_BATCH: usize = 100;
pub const INTERNAL_EVENT_MAX_BATCH_BYTES: usize = 10 * 1024 * 1024;
pub const INTERNAL_EVENT_MAX_QUEUE: usize = 200;
pub const DELIVERY_MAX_BATCH: usize = 64;
pub const MAX_INTERNAL_EVENTS_PER_PAGE: usize = 100;
pub const READ_INTERNAL_EVENTS_MAX_RETRIES: u32 = 10;

/// Why `initialize` refused to start the worker protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CcrInitFailReason {
    NoAuthHeaders,
    MissingEpoch,
    WorkerRegisterFailed,
}

impl std::fmt::Display for CcrInitFailReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            CcrInitFailReason::NoAuthHeaders => "no_auth_headers",
            CcrInitFailReason::MissingEpoch => "missing_epoch",
            CcrInitFailReason::WorkerRegisterFailed => "worker_register_failed",
        };
        f.write_str(text)
    }
}

/// API message ID → per-block accumulated text chunks (source
/// `StreamAccumulatorState`).
#[derive(Default)]
pub struct StreamAccumulatorState {
    pub by_message: HashMap<String, HashMap<usize, Vec<String>>>,
    pub scope_to_message: HashMap<String, String>,
}

pub fn create_stream_accumulator() -> StreamAccumulatorState {
    StreamAccumulatorState::default()
}

/// `{session_id}:{parent_tool_use_id}` — content_block_delta events carry no
/// message ID, so the active message is tracked per scope.
pub fn scope_key(session_id: &str, parent_tool_use_id: Option<&str>) -> String {
    format!("{}:{}", session_id, parent_tool_use_id.unwrap_or(""))
}

/// Accumulate text_delta stream_events into full-so-far snapshots per content
/// block (source: `accumulateStreamEvents`). Each flush emits ONE event per
/// touched block carrying the FULL text from the block start; the snapshot
/// reuses the first text_delta UUID seen for that block in this flush so
/// server-side idempotency stays stable across retries.
pub fn accumulate_stream_events(
    buffer: &[Value],
    state: &mut StreamAccumulatorState,
) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    let mut touched: HashMap<(String, usize), usize> = HashMap::new();
    for msg in buffer {
        let event_type = msg
            .pointer("/event/type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match event_type {
            "message_start" => {
                let id = msg
                    .pointer("/event/message/id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let scope = scope_key(
                    msg.get("session_id").and_then(Value::as_str).unwrap_or(""),
                    msg.get("parent_tool_use_id").and_then(Value::as_str),
                );
                if let Some(prev_id) = state.scope_to_message.insert(scope, id.clone()) {
                    state.by_message.remove(&prev_id);
                }
                state.by_message.entry(id).or_default();
                out.push(msg.clone());
            }
            "content_block_delta" => {
                if msg.pointer("/event/delta/type").and_then(Value::as_str) != Some("text_delta") {
                    out.push(msg.clone());
                    continue;
                }
                let scope = scope_key(
                    msg.get("session_id").and_then(Value::as_str).unwrap_or(""),
                    msg.get("parent_tool_use_id").and_then(Value::as_str),
                );
                let Some(message_id) = state.scope_to_message.get(&scope).cloned() else {
                    // Delta without a preceding message_start (reconnect
                    // mid-stream): pass through raw — no chunks to snapshot.
                    out.push(msg.clone());
                    continue;
                };
                let index = msg
                    .pointer("/event/index")
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as usize;
                let text = msg
                    .pointer("/event/delta/text")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let Some(blocks) = state.by_message.get_mut(&message_id) else {
                    out.push(msg.clone());
                    continue;
                };
                let chunks = blocks.entry(index).or_default();
                chunks.push(text);
                let snapshot_text = chunks.join("");
                let snapshot_key = (message_id, index);
                match touched.get(&snapshot_key) {
                    Some(&out_index) => {
                        // Rewrite the snapshot emitted earlier in this flush.
                        if let Some(existing) = out.get_mut(out_index) {
                            existing["event"]["delta"]["text"] = json!(snapshot_text);
                        }
                    }
                    None => {
                        let snapshot = json!({
                            "type": "stream_event",
                            "uuid": msg.get("uuid").cloned().unwrap_or(Value::Null),
                            "session_id": msg.get("session_id").cloned().unwrap_or(Value::Null),
                            "parent_tool_use_id": msg.get("parent_tool_use_id").cloned().unwrap_or(Value::Null),
                            "event": {
                                "type": "content_block_delta",
                                "index": index,
                                "delta": {"type": "text_delta", "text": snapshot_text},
                            }
                        });
                        touched.insert(snapshot_key, out.len());
                        out.push(snapshot);
                    }
                }
            }
            _ => out.push(msg.clone()),
        }
    }
    out
}

/// Clear accumulator entries for a completed assistant message (source
/// `clearStreamAccumulatorForMessage`).
pub fn clear_stream_accumulator_for_message(
    state: &mut StreamAccumulatorState,
    session_id: &str,
    parent_tool_use_id: Option<&str>,
    message_id: &str,
) {
    state.by_message.remove(message_id);
    let scope = scope_key(session_id, parent_tool_use_id);
    if state.scope_to_message.get(&scope) == Some(&message_id.to_string()) {
        state.scope_to_message.remove(&scope);
    }
}

/// Decode a JWT expiry for the 401/403 diagnostic path.
fn jwt_expired_or_invalid(token: &str) -> bool {
    crate::jwt_utils::decode_jwt_expiry(token).is_none_or(|exp| exp <= now_unix_secs())
}

fn now_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0)
}

type EpochMismatchHandler = Box<dyn Fn() + Send + Sync>;
type AuthHeaders = Arc<dyn Fn() -> Vec<(String, String)> + Send + Sync>;

struct CcrShared {
    session_base_url: String,
    auth: AuthHeaders,
    on_epoch_mismatch: EpochMismatchHandler,
    worker_epoch: AtomicU64,
    consecutive_auth_failures: AtomicU64,
    closed: AtomicBool,
}

pub struct CcrClient {
    shared: Arc<CcrShared>,
    session_base_url: String,
    session_id: String,
    heartbeat_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    heartbeat_interval_ms: u64,
    heartbeat_jitter_fraction: f64,
    stream_event_buffer: Arc<Mutex<Vec<Value>>>,
    stream_timer_running: Arc<AtomicBool>,
    accumulator: Arc<Mutex<StreamAccumulatorState>>,
    worker_state: Arc<WorkerStateUploader>,
    event_uploader: Arc<SerialBatchEventUploader<Value>>,
    internal_event_uploader: SerialBatchEventUploader<Value>,
    delivery_uploader: Arc<SerialBatchEventUploader<Value>>,
    http: reqwest::Client,
}

impl CcrClient {
    /// Build a client bound to an SSE ingress transport's session URL.
    pub fn new(
        _transport: &SseTransport,
        session_url: &str,
        auth: AuthHeaders,
        on_epoch_mismatch: Option<EpochMismatchHandler>,
        heartbeat_interval_ms: Option<u64>,
        heartbeat_jitter_fraction: Option<f64>,
    ) -> Self {
        let session_base_url = session_url.trim_end_matches('/').to_string();
        let shared = Arc::new(CcrShared {
            session_base_url: session_base_url.clone(),
            auth,
            on_epoch_mismatch: on_epoch_mismatch.unwrap_or_else(|| Box::new(|| {})),
            worker_epoch: AtomicU64::new(0),
            consecutive_auth_failures: AtomicU64::new(0),
            closed: AtomicBool::new(false),
        });
        let sender_shared = shared.clone();
        let client_events_send: UploaderSend<Value> = Arc::new(move |batch| {
            let shared = sender_shared.clone();
            Box::pin(async move {
                shared
                    .request_post("/worker/events", &json!({
                        "worker_epoch": shared.worker_epoch.load(Ordering::SeqCst),
                        "events": batch.to_vec()
                    }))
                    .await
            })
        });
        let mut client_events_config = UploaderConfig::new(client_events_send);
        client_events_config.max_batch_size = usize::MAX;
        client_events_config.max_queue_size = CLIENT_EVENT_MAX_QUEUE;
        client_events_config.base_delay_ms = CLIENT_EVENT_BASE_DELAY_MS;
        client_events_config.max_delay_ms = CLIENT_EVENT_MAX_DELAY_MS;
        client_events_config.jitter_ms = CLIENT_EVENT_JITTER_MS;

        let sender_shared = shared.clone();
        let internal_events_send: UploaderSend<Value> = Arc::new(move |batch| {
            let shared = sender_shared.clone();
            Box::pin(async move {
                shared
                    .request_post(
                        "/worker/internal-events",
                        &json!({
                            "worker_epoch": shared.worker_epoch.load(Ordering::SeqCst),
                            "events": batch.to_vec()
                        }),
                    )
                    .await
            })
        });
        let mut internal_config = UploaderConfig::new(internal_events_send);
        internal_config.max_batch_size = INTERNAL_EVENT_MAX_BATCH;
        internal_config.max_batch_bytes = Some(INTERNAL_EVENT_MAX_BATCH_BYTES);
        internal_config.max_queue_size = INTERNAL_EVENT_MAX_QUEUE;
        internal_config.base_delay_ms = CLIENT_EVENT_BASE_DELAY_MS;
        internal_config.max_delay_ms = CLIENT_EVENT_MAX_DELAY_MS;
        internal_config.jitter_ms = CLIENT_EVENT_JITTER_MS;

        let sender_shared = shared.clone();
        let delivery_send: UploaderSend<Value> = Arc::new(move |batch| {
            let shared = sender_shared.clone();
            let updates: Vec<Value> = batch
                .to_vec()
                .into_iter()
                .map(|entry| {
                    json!({
                        "event_id": entry.get("eventId").cloned().unwrap_or(Value::Null),
                        "status": entry.get("status").cloned().unwrap_or(Value::Null),
                    })
                })
                .collect();
            Box::pin(async move {
                shared
                    .request_post("/worker/events/delivery", &json!({
                        "worker_epoch": shared.worker_epoch.load(Ordering::SeqCst),
                        "updates": updates
                    }))
                    .await
            })
        });
        let mut delivery_config = UploaderConfig::new(delivery_send);
        delivery_config.max_batch_size = DELIVERY_MAX_BATCH;
        delivery_config.max_queue_size = DELIVERY_MAX_BATCH;
        delivery_config.base_delay_ms = CLIENT_EVENT_BASE_DELAY_MS;
        delivery_config.max_delay_ms = CLIENT_EVENT_MAX_DELAY_MS;
        delivery_config.jitter_ms = CLIENT_EVENT_JITTER_MS;

        let worker_state_send: WorkerStateSend = {
            let shared = shared.clone();
            Arc::new(move |body| {
                let shared = shared.clone();
                Box::pin(async move {
                    let mut patch = body;
                    if let Some(fields) = patch.as_object_mut() {
                        fields.insert(
                            "worker_epoch".to_owned(),
                            json!(shared.worker_epoch.load(Ordering::SeqCst)),
                        );
                    }
                    shared.request_put("/worker", &patch).await.is_ok()
                })
            })
        };

        Self {
            http: reqwest::Client::new(),
            session_base_url,
            session_id: String::new(),
            heartbeat_task: Mutex::new(None),
            heartbeat_interval_ms: heartbeat_interval_ms.unwrap_or(DEFAULT_HEARTBEAT_INTERVAL_MS),
            heartbeat_jitter_fraction: heartbeat_jitter_fraction
                .unwrap_or(DEFAULT_HEARTBEAT_JITTER_FRACTION),
            stream_event_buffer: Arc::new(Mutex::new(Vec::new())),
            stream_timer_running: Arc::new(AtomicBool::new(false)),
            accumulator: Arc::new(Mutex::new(create_stream_accumulator())),
            worker_state: Arc::new(WorkerStateUploader::new(worker_state_send, 1_000, 30_000)),
            event_uploader: Arc::new(SerialBatchEventUploader::new(client_events_config)),
            internal_event_uploader: SerialBatchEventUploader::new(internal_config),
            delivery_uploader: Arc::new(SerialBatchEventUploader::new(delivery_config)),
            shared,
        }
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn get_worker_epoch(&self) -> u64 {
        self.shared.worker_epoch.load(Ordering::SeqCst)
    }

    /// Start only after the parent bridge has registered an epoch. The
    /// original CCRClient.initialize performs a synchronous worker PUT and
    /// starts heartbeats only after the server accepts the epoch.
    pub async fn initialize(
        &mut self,
        session_id: &str,
        epoch: Option<u64>,
    ) -> Result<Option<Value>> {
        if (self.shared.auth)().is_empty() {
            bail!("{}", CcrInitFailReason::NoAuthHeaders);
        }
        let epoch = match epoch {
            Some(value) => value,
            None => std::env::var("ATLAS_WORKER_EPOCH")
                .ok()
                .and_then(|raw| raw.parse::<u64>().ok())
                .ok_or(CcrInitFailReason::MissingEpoch)
                .map_err(|reason| anyhow!("{reason}"))?,
        };
        if self.shared.closed.load(Ordering::SeqCst) {
            bail!("CCR client closed");
        }
        self.shared.worker_epoch.store(epoch, Ordering::SeqCst);
        // The GET may finish before the PUT, but its state is only accepted
        // when the subsequent PUT has succeeded (source: restoredPromise).
        let (restore, registered) = tokio::join!(
            self.shared.request_get("/worker"),
            self.shared.request_put(
                "/worker",
                &json!({
                    "worker_status": "idle",
                    "worker_epoch": epoch,
                    "external_metadata": {
                        "pending_action": null,
                        "task_summary": null
                    }
                }),
            ),
        );
        registered.map_err(|error| {
            anyhow!("{}: {error}", CcrInitFailReason::WorkerRegisterFailed)
        })?;
        self.session_id = session_id.to_owned();
        self.start_heartbeat();
        Ok(restore.ok().and_then(|value| {
            value.pointer("/worker/external_metadata").cloned()
        }))
    }

    /// Heartbeats are owned by this client and are cancelled on close.
    /// A second start replaces the prior timer instead of leaking a task.
    pub fn start_heartbeat(&self) {
        let mut task = self.heartbeat_task.lock().expect("CCR heartbeat task");
        if let Some(previous) = task.take() {
            previous.abort();
        }
        if self.shared.closed.load(Ordering::SeqCst) {
            return;
        }
        let shared = Arc::clone(&self.shared);
        let session_id = self.session_id.clone();
        let interval_ms = self.heartbeat_interval_ms;
        let jitter_fraction = self.heartbeat_jitter_fraction;
        *task = Some(tokio::spawn(async move {
            loop {
                let jitter = interval_ms as f64
                    * jitter_fraction
                    * (2.0 * rand_unit() - 1.0);
                let duration = (interval_ms as f64 + jitter).max(1.0) as u64;
                tokio::time::sleep(Duration::from_millis(duration)).await;
                if shared.closed.load(Ordering::SeqCst) {
                    return;
                }
                let _ = shared.request_post(
                    "/worker/heartbeat",
                    &json!({
                        "session_id": session_id,
                        "worker_epoch": shared.worker_epoch.load(Ordering::SeqCst)
                    }),
                ).await;
            }
        }));
    }

    /// Write one stdout message as a client event. stream_event messages are
    /// held in a 100ms buffer and coalesced; non-stream writes flush first.
    pub async fn write_event(&self, message: &Value) {
        match message.get("type").and_then(Value::as_str) {
            Some("stream_event") => {
                self.stream_event_buffer
                    .lock()
                    .expect("stream buffer")
                    .push(message.clone());
                if !self.stream_timer_running.swap(true, Ordering::SeqCst) {
                    let buffer = Arc::clone(&self.stream_event_buffer);
                    let accumulator = Arc::clone(&self.accumulator);
                    let uploader = Arc::clone(&self.event_uploader);
                    let timer = Arc::clone(&self.stream_timer_running);
                    tokio::spawn(async move {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        let batch = std::mem::take(&mut *buffer.lock().expect("stream buffer"));
                        timer.store(false, Ordering::SeqCst);
                        if !batch.is_empty() {
                            let events = {
                                let mut state = accumulator.lock().expect("accumulator");
                                accumulate_stream_events(&batch, &mut state)
                            };
                            for event in events {
                                uploader
                                    .enqueue(vec![CcrClient::to_client_event(&event)])
                                    .await;
                            }
                        }
                    });
                }
                return;
            }
            Some("assistant") => {
                self.flush_stream_event_buffer().await;
                let mut accumulator = self.accumulator.lock().expect("accumulator");
                clear_stream_accumulator_for_message(
                    &mut accumulator,
                    message
                        .get("session_id")
                        .and_then(Value::as_str)
                        .unwrap_or(""),
                    message.get("parent_tool_use_id").and_then(Value::as_str),
                    message
                        .pointer("/message/id")
                        .and_then(Value::as_str)
                        .unwrap_or_default(),
                );
            }
            _ => self.flush_stream_event_buffer().await,
        }
        self.event_uploader
            .enqueue(vec![Self::to_client_event(message)])
            .await;
    }

    fn to_client_event(message: &Value) -> Value {
        let uuid = match message.get("uuid").and_then(Value::as_str) {
            Some(existing) => json!(existing),
            None => json!(uuid::Uuid::new_v4().to_string()),
        };
        let mut payload = message.clone();
        payload["uuid"] = uuid;
        json!({"payload": payload})
    }

    /// Drain the stream_event delay buffer through the text accumulator.
    pub async fn flush_stream_event_buffer(&self) {
        let buffered: Vec<Value> =
            std::mem::take(&mut *self.stream_event_buffer.lock().expect("stream buffer"));
        if buffered.is_empty() {
            return;
        }
        let events = {
            let mut accumulator = self.accumulator.lock().expect("accumulator");
            accumulate_stream_events(&buffered, &mut accumulator)
        };
        for event in events {
            self.event_uploader
                .enqueue(vec![Self::to_client_event(&event)])
                .await;
        }
    }

    pub async fn flush(&self) {
        self.flush_stream_event_buffer().await;
        self.event_uploader.flush().await;
    }

    /// Queue one internal event (system diagnostics / worker signals).
    pub async fn write_internal_event(
        &self,
        event_type: &str,
        payload: &Value,
        event_metadata: Option<&Value>,
    ) {
        let event = json!({
            "event_id": uuid::Uuid::new_v4().to_string(),
            "event_type": event_type,
            "payload": payload,
            "event_metadata": event_metadata.cloned().unwrap_or(Value::Null),
            "is_compaction": false,
            "created_at": chrono_now_iso(),
        });
        self.internal_event_uploader.enqueue(vec![event]).await;
    }

    /// Depth of the internal-event queue (shutdown backpressure signal).
    pub fn internal_events_pending(&self) -> usize {
        self.internal_event_uploader.pending_count()
    }

    /// Source paginatedGet returns null if any page fails after retries.
    /// Here a Result error serves the same purpose: never return a partial
    /// event stream as though session recovery completed successfully.
    pub async fn read_internal_events(
        &self,
        limit: usize,
        cursor: Option<&str>,
    ) -> Result<Vec<Value>> {
        self.read_internal_events_with_retry_limit(limit, cursor, READ_INTERNAL_EVENTS_MAX_RETRIES)
            .await
    }

    fn internal_events_path(&self, limit: usize, cursor: Option<&str>) -> Result<String> {
        let mut url =
            reqwest::Url::parse(&format!("{}/worker/internal-events", self.session_base_url))?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair(
                "limit",
                &limit.min(MAX_INTERNAL_EVENTS_PER_PAGE).to_string(),
            );
            if let Some(cursor) = cursor {
                pairs.append_pair("cursor", cursor);
            }
        }
        Ok(format!(
            "/worker/internal-events?{}",
            url.query().unwrap_or_default()
        ))
    }

    async fn read_internal_events_with_retry_limit(
        &self,
        limit: usize,
        cursor: Option<&str>,
        max_failures: u32,
    ) -> Result<Vec<Value>> {
        let mut collected: Vec<Value> = Vec::new();
        let mut next_cursor = cursor.map(str::to_string);
        let mut failures = 0_u32;
        loop {
            let path = self.internal_events_path(limit, next_cursor.as_deref())?;
            match self.shared.request_get(&path).await {
                Ok(body) => {
                    failures = 0;
                    match body.get("data") {
                        Some(Value::Array(data)) => collected.extend(data.iter().cloned()),
                        None | Some(Value::Null) => {}
                        Some(_) => bail!("CCR internal-events response data must be an array"),
                    }
                    next_cursor = body
                        .get("next_cursor")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    if next_cursor.is_none() {
                        return Ok(collected);
                    }
                }
                Err(error) => {
                    failures = failures.saturating_add(1);
                    if failures >= max_failures {
                        bail!(
                            "CCR internal-event recovery failed after {failures} attempts; {} uncommitted entries: {error}",
                            collected.len()
                        );
                    }
                    let delay = super::serial_batch_event_uploader::retry_delay(
                        failures,
                        None,
                        CLIENT_EVENT_BASE_DELAY_MS,
                        CLIENT_EVENT_MAX_DELAY_MS,
                        CLIENT_EVENT_JITTER_MS,
                    );
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    /// Acknowledge one ingress event's delivery state.
    pub fn report_delivery(&self, event_id: &str, status: &str) {
        let event_id = event_id.to_string();
        let status = status.to_string();
        tokio::spawn({
            let uploader = Arc::clone(&self.delivery_uploader);
            async move {
                uploader
                    .enqueue(vec![json!({"eventId": event_id, "status": status})])
                    .await;
            }
        });
    }

    /// Stop heartbeat-side work and close all uploaders. Buffered stream
    /// events are dropped — call `flush` first if delivery is required.
    pub fn close(&self) {
        self.shared.closed.store(true, Ordering::SeqCst);
        if let Some(task) = self.heartbeat_task.lock().expect("CCR heartbeat task").take() {
            task.abort();
        }
        self.stream_event_buffer
            .lock()
            .expect("stream buffer")
            .clear();
        self.event_uploader.close();
        self.internal_event_uploader.close();
        self.delivery_uploader.close();
        self.worker_state.close();
    }
}

impl CcrShared {
    async fn request_post(
        &self,
        path: &str,
        body: &Value,
    ) -> std::result::Result<(), UploaderError> {
        self.request_raw("POST", path, Some(body)).await
    }

    async fn request_put(&self, path: &str, body: &Value) -> Result<Value> {
        self.request_raw_value("PUT", path, Some(body)).await
    }

    async fn request_get(&self, path: &str) -> Result<Value> {
        self.request_raw_value("GET", path, None).await
    }

    async fn request_raw(
        &self,
        method: &str,
        path: &str,
        body: Option<&Value>,
    ) -> std::result::Result<(), UploaderError> {
        self.request_raw_value(method, path, body)
            .await
            .map(|_| ())
            .map_err(|error| match error.downcast::<UploaderError>() {
                Ok(uploader_error) => uploader_error,
                Err(_) => UploaderError::Retryable {
                    retry_after_ms: None,
                },
            })
    }

    async fn request_raw_value(
        &self,
        method: &str,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Value> {
        if self.closed.load(Ordering::SeqCst) {
            bail!("CCR client closed");
        }
        let url = format!("{}{}", self.session_base_url, path);
        let mut request = match method {
            "POST" => reqwest::Client::new().post(&url),
            "PUT" => reqwest::Client::new().put(&url),
            _ => reqwest::Client::new().get(&url),
        };
        for (name, value) in (self.auth)() {
            request = request.header(name, value);
        }
        if let Some(body) = body {
            request = request
                .header("Content-Type", "application/json")
                .body(crate::ndjson::stringify_value(body));
        }
        let request_timeout = if method == "GET" {
            Duration::from_secs(30)
        } else {
            Duration::from_secs(10)
        };
        let response = request.timeout(request_timeout).send().await?;
        let status = response.status().as_u16();
        if status == 409 {
            (self.on_epoch_mismatch)();
            bail!("epoch mismatch (409): a newer worker epoch superseded ours");
        }
        if status == 401 || status == 403 {
            let failures = self
                .consecutive_auth_failures
                .fetch_add(1, Ordering::SeqCst)
                + 1;
            let token = (self.auth)()
                .into_iter()
                .find(|(name, _)| name.eq_ignore_ascii_case("authorization"))
                .map(|(_, value)| value)
                .unwrap_or_default();
            let token = token.strip_prefix("Bearer ").unwrap_or(&token);
            if jwt_expired_or_invalid(token) || failures >= AUTH_FAILURE_THRESHOLD as u64 {
                bail!("auth failures exhausted after {failures} attempts (status {status})");
            }
            return Err(anyhow::Error::new(UploaderError::Retryable {
                retry_after_ms: None,
            })
            .context(format!("auth failure {failures}")));
        }
        if status == 429 {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok())
                .map(|secs| secs * 1000);
            return Err(anyhow::Error::new(UploaderError::Retryable {
                retry_after_ms: retry_after,
            })
            .context("rate limited (429)"));
        }
        if (200..300).contains(&status) {
            self.consecutive_auth_failures.store(0, Ordering::SeqCst);
            let text = response.text().await.unwrap_or_default();
            return Ok(serde_json::from_str(&text).unwrap_or(json!({})));
        }
        bail!("CCR request {method} {path} failed with status {status}")
    }
}

fn rand_unit() -> f64 {
    let mut bytes = [0_u8; 8];
    getrandom::getrandom(&mut bytes).ok();
    let value = u64::from_be_bytes(bytes) % 10_000;
    value as f64 / 10_000.0
}

fn chrono_now_iso() -> String {
    // Lightweight RFC3339 timestamp without an extra dependency.
    let secs = now_unix_secs();
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (year, month, day) = civil_from_days(days as i64);
    format!("{year:04}-{month:02}-{day:02}T{h:02}:{m:02}:{s:02}Z")
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    // Howard Hinnant's algorithm, public domain.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = if m <= 2 { y + 1 } else { y };
    (year, m, d)
}

/// Parse a base64url work-secret body for tests and offline tooling.
pub fn decode_secret_body(secret: &str) -> Result<Value> {
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(secret)?;
    Ok(serde_json::from_slice(&decoded)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stream_event(uuid: &str, session: &str, index: u64, text: &str) -> Value {
        json!({
            "type": "stream_event",
            "uuid": uuid,
            "session_id": session,
            "parent_tool_use_id": Value::Null,
            "event": {
                "type": "content_block_delta",
                "index": index,
                "delta": {"type": "text_delta", "text": text}
            }
        })
    }

    #[test]
    fn text_deltas_coalesce_into_full_snapshots() {
        let mut state = create_stream_accumulator();
        let start = json!({
            "type": "stream_event",
            "uuid": "u0",
            "session_id": "s",
            "parent_tool_use_id": Value::Null,
            "event": {"type": "message_start", "index": 0, "message": {"id": "msg_1"}}
        });
        let buffer = vec![
            start,
            stream_event("u1", "s", 0, "Hello"),
            stream_event("u2", "s", 0, ", "),
            stream_event("u3", "s", 0, "world"),
        ];
        let out = accumulate_stream_events(&buffer, &mut state);
        // message_start passes through; the three deltas collapse into ONE
        // full-so-far snapshot keyed by the first delta's UUID.
        assert_eq!(out.len(), 2);
        assert_eq!(out[0]["event"]["type"], "message_start");
        assert_eq!(out[1]["uuid"], "u1");
        assert_eq!(out[1]["event"]["delta"]["text"], "Hello, world");
    }

    #[test]
    fn deltas_without_message_start_pass_through() {
        let mut state = create_stream_accumulator();
        let buffer = vec![stream_event("u1", "s", 0, "orphan")];
        let out = accumulate_stream_events(&buffer, &mut state);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0]["event"]["delta"]["text"], "orphan");
    }

    #[test]
    fn assistant_completion_clears_accumulator() {
        let mut state = create_stream_accumulator();
        let start = json!({
            "type": "stream_event", "uuid": "u0", "session_id": "s",
            "parent_tool_use_id": Value::Null,
            "event": {"type": "message_start", "index": 0, "message": {"id": "msg_1"}}
        });
        let _ = accumulate_stream_events(&[start, stream_event("u1", "s", 0, "hi")], &mut state);
        assert!(state.by_message.contains_key("msg_1"));
        clear_stream_accumulator_for_message(&mut state, "s", None, "msg_1");
        assert!(!state.by_message.contains_key("msg_1"));
        assert!(!state.scope_to_message.contains_key("s:"));
    }

    #[test]
    fn non_text_deltas_pass_through() {
        let mut state = create_stream_accumulator();
        let tool_use = json!({
            "type": "stream_event", "uuid": "u1", "session_id": "s",
            "parent_tool_use_id": Value::Null,
            "event": {"type": "content_block_start", "index": 0,
                "content_block": {"type": "tool_use", "name": "Read"}}
        });
        let out = accumulate_stream_events(&[tool_use], &mut state);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0]["event"]["type"], "content_block_start");
    }

    #[test]
    fn scope_keys_distinguish_parent_contexts() {
        assert_eq!(scope_key("s1", None), "s1:");
        assert_eq!(scope_key("s1", Some("agent_1")), "s1:agent_1");
        assert_ne!(scope_key("s1", None), scope_key("s1", Some("agent_1")));
    }

    #[test]
    fn internal_event_cursor_is_query_encoded_and_roundtrips() {
        let client = ccr_client_for_test();
        let cursor = "page/1?start=二&more=two words";
        let path = client.internal_events_path(100, Some(cursor)).unwrap();
        let url = reqwest::Url::parse(&format!("http://127.0.0.1:9{path}")).unwrap();
        let params = url
            .query_pairs()
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(
            params.get("cursor").map(|value| value.as_ref()),
            Some(cursor)
        );
        assert_eq!(params.get("limit").map(|value| value.as_ref()), Some("100"));
        client.close();
    }

    #[tokio::test]
    async fn internal_event_recovery_failure_is_never_reported_as_empty_success() {
        let client = ccr_client_for_test();
        let error = client
            .read_internal_events_with_retry_limit(100, None, 1)
            .await
            .expect_err("unreachable ingress must fail recovery");
        assert!(error
            .to_string()
            .contains("recovery failed after 1 attempts"));
        client.close();
    }

    #[tokio::test]
    async fn init_rejects_missing_credentials_without_background_heartbeat() {
        let transport = SseTransport::new(
            "http://127.0.0.1:9/session/s1/events/stream".to_owned(),
            vec![],
            None,
            Arc::new(Vec::new),
        );
        let mut client = CcrClient::new(
            &transport,
            "http://127.0.0.1:9/session/s1",
            Arc::new(Vec::new),
            None,
            Some(1),
            Some(0.0),
        );
        let error = client.initialize("s1", Some(9)).await.unwrap_err();
        assert!(error.to_string().contains("no_auth_headers"));
        assert!(client.heartbeat_task.lock().unwrap().is_none());
        client.close();
    }

    #[tokio::test]
    async fn rejected_worker_put_does_not_start_heartbeat() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            for _ in 0..2 {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = [0_u8; 2048];
                let length = socket.read(&mut request).await.unwrap();
                let message = String::from_utf8_lossy(&request[..length]);
                let status = if message.starts_with("PUT ") {
                    "409 Conflict"
                } else {
                    "200 OK"
                };
                let body = if status == "200 OK" { "{}" } else { "{}" };
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                socket.write_all(response.as_bytes()).await.unwrap();
            }
        });
        let transport = SseTransport::new(
            format!("http://{addr}/session/s1/events/stream"),
            vec![],
            None,
            Arc::new(Vec::new),
        );
        let mut client = CcrClient::new(
            &transport,
            &format!("http://{addr}/session/s1"),
            Arc::new(|| vec![("Authorization".into(), "Bearer test".into())]),
            None,
            Some(10),
            Some(0.0),
        );
        let result = client.initialize("s1", Some(12)).await;
        assert!(result.is_err(), "worker epoch conflict must fail initialization");
        assert!(client.heartbeat_task.lock().unwrap().is_none());
        client.close();
        tokio::time::timeout(Duration::from_secs(3), server).await.unwrap().unwrap();
    }

    #[test]
    fn init_failure_reasons_render() {
        assert_eq!(
            CcrInitFailReason::NoAuthHeaders.to_string(),
            "no_auth_headers"
        );
        assert_eq!(CcrInitFailReason::MissingEpoch.to_string(), "missing_epoch");
        assert_eq!(
            CcrInitFailReason::WorkerRegisterFailed.to_string(),
            "worker_register_failed"
        );
    }

    #[test]
    fn civil_date_math() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
    }

    #[tokio::test]
    async fn write_event_buffers_stream_events_until_flush() {
        let client = ccr_client_for_test();
        client
            .write_event(&stream_event("u1", "s", 0, "hello"))
            .await;
        // Buffer holds the event, nothing enqueued yet.
        assert_eq!(client.stream_event_buffer.lock().unwrap().len(), 1);
        client.flush_stream_event_buffer().await;
        assert!(client.stream_event_buffer.lock().unwrap().is_empty());
        client.close();
    }

    fn ccr_client_for_test() -> Arc<CcrClient> {
        let transport = SseTransport::new(
            "http://127.0.0.1:9/session/s1/events/stream".to_string(),
            vec![],
            Some("s1".to_string()),
            Arc::new(|| vec![]),
        );
        let client = Arc::new(CcrClient::new(
            &transport,
            "http://127.0.0.1:9/session/s1",
            Arc::new(|| vec![("authorization".to_string(), "Bearer x".to_string())]),
            None,
            Some(1000),
            Some(0.0),
        ));
        client
    }
}
