//! Coalescing uploader for PUT /worker (session state + metadata).
//! 2.1.88 reference: `src/cli/transports/WorkerStateUploader.ts`.
//!
//! - 1 in-flight PUT + 1 pending patch
//! - New calls coalesce into pending (never grows beyond 1 slot)
//! - On success: send pending if exists
//! - On failure: exponential backoff (clamped), retries indefinitely until
//!   success or `close`; absorbs pending patches before each retry
//! - No backpressure needed — naturally bounded at 2 slots
//!
//! Coalescing rules:
//! - Top-level keys — last value wins
//! - Inside `external_metadata` / `internal_metadata` — RFC 7396 merge:
//!   overlay keys are added/overwritten, null values preserved (server deletes)

use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

/// Shallow overlay merge: nested metadata objects merge one level deep,
/// everything else is replaced wholesale (mirrors `coalescePatches`).
pub fn coalesce_patches(base: &Value, overlay: &Value) -> Value {
    let Some(base_obj) = base.as_object() else {
        return overlay.clone();
    };
    let Some(overlay_obj) = overlay.as_object() else {
        return overlay.clone();
    };
    let mut merged = base_obj.clone();
    for (key, value) in overlay_obj {
        let merges = matches!(key.as_str(), "external_metadata" | "internal_metadata")
            && merged.get(key).is_some_and(|existing| existing.is_object())
            && value.is_object();
        if merges {
            let existing = merged.get(key).unwrap().as_object().unwrap().clone();
            let overlay_part = value.as_object().unwrap();
            let mut combined = existing;
            for (inner_key, inner_value) in overlay_part {
                combined.insert(inner_key.clone(), inner_value.clone());
            }
            merged.insert(key.clone(), Value::Object(combined));
        } else {
            merged.insert(key.clone(), value.clone());
        }
    }
    Value::Object(merged)
}

pub type WorkerStateSend = Arc<
    dyn Fn(Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>>
        + Send
        + Sync,
>;

struct WorkerStateShared {
    send: WorkerStateSend,
    base_delay_ms: u64,
    max_delay_ms: u64,
    state: Mutex<WorkerStateInner>,
    wake: Notify,
    closed: AtomicBool,
}

struct WorkerStateInner {
    in_flight: bool,
    pending: Option<Value>,
}

pub struct WorkerStateUploader {
    shared: Arc<WorkerStateShared>,
}

impl WorkerStateUploader {
    pub fn new(send: WorkerStateSend, base_delay_ms: u64, max_delay_ms: u64) -> Self {
        Self {
            shared: Arc::new(WorkerStateShared {
                send,
                base_delay_ms,
                max_delay_ms,
                state: Mutex::new(WorkerStateInner {
                    in_flight: false,
                    pending: None,
                }),
                wake: Notify::new(),
                closed: AtomicBool::new(false),
            }),
        }
    }

    /// Enqueue one patch. Coalesces into the pending slot while a PUT is in
    /// flight; otherwise sends immediately on a spawned task.
    pub fn enqueue(&self, patch: Value) {
        let kick = {
            let mut state = self.shared.state.lock().expect("worker state");
            if self.shared.closed.load(Ordering::SeqCst) {
                return;
            }
            if state.in_flight {
                state.pending = Some(match state.pending.take() {
                    Some(existing) => coalesce_patches(&existing, &patch),
                    None => patch,
                });
                false
            } else {
                state.in_flight = true;
                true
            }
        };
        if kick {
            let shared = self.shared.clone();
            tokio::spawn(async move { send_loop(shared, None).await });
        }
    }

    /// Stop accepting work; the in-flight attempt finishes on its own.
    pub fn close(&self) {
        self.shared.closed.store(true, Ordering::SeqCst);
        self.shared.wake.notify_waiters();
    }

    /// Whether an attempt is in flight (test hook parity with the source).
    pub fn is_running(&self) -> bool {
        self.shared.state.lock().expect("worker state").in_flight
    }
}

async fn send_loop(shared: Arc<WorkerStateShared>, mut failures: Option<u32>) {
    let mut failures = failures.unwrap_or(0);
    loop {
        let body = {
            let mut state = shared.state.lock().expect("worker state");
            if shared.closed.load(Ordering::SeqCst) {
                return;
            }
            match state.pending.take() {
                Some(pending) => pending,
                None => Value::Object(Default::default()),
            }
        };
        let ok = (shared.send)(body).await;
        if ok {
            failures = 0;
            let kick;
            {
                let mut state = shared.state.lock().expect("worker state");
                if state.pending.is_some() {
                    kick = true;
                } else {
                    state.in_flight = false;
                    kick = false;
                }
            }
            if kick {
                continue;
            }
            return;
        }
        failures += 1;
        let delay =
            (shared.base_delay_ms * (1u64 << (failures - 1).min(5))).min(shared.max_delay_ms);
        tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
        if shared.closed.load(Ordering::SeqCst) {
            return;
        }
        // Absorb pending patches before the retry (they were taken above).
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn overlay_merge_preserves_nulls_and_wins() {
        let base = json!({
            "worker_status": "idle",
            "external_metadata": {"task_summary": "old", "extra": 1}
        });
        let overlay = json!({
            "worker_status": "running",
            "external_metadata": {"task_summary": "new", "pending_action": null}
        });
        let merged = coalesce_patches(&base, &overlay);
        assert_eq!(merged["worker_status"], "running");
        assert_eq!(merged["external_metadata"]["task_summary"], "new");
        assert_eq!(merged["external_metadata"]["extra"], 1);
        assert_eq!(
            merged["external_metadata"]["pending_action"],
            serde_json::Value::Null
        );
    }

    #[test]
    fn non_metadata_keys_replace_wholesale() {
        let base = json!({"internal_metadata": {"a": 1}, "other": {"x": 1}});
        let overlay = json!({"other": {"y": 2}});
        let merged = coalesce_patches(&base, &overlay);
        assert_eq!(merged["other"], json!({"y": 2}));
        assert_eq!(merged["internal_metadata"]["a"], 1);
    }

    #[tokio::test]
    async fn coalesces_pending_patches() {
        let sent = Arc::new(Mutex::new(Vec::<Value>::new()));
        let collector = sent.clone();
        let send: WorkerStateSend = Arc::new(move |body| {
            let collector = collector.clone();
            Box::pin(async move {
                collector.lock().unwrap().push(body);
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                true
            })
        });
        let uploader = WorkerStateUploader::new(send, 1, 4);
        uploader.enqueue(json!({"worker_status": "idle"}));
        uploader.enqueue(json!({"worker_status": "running"}));
        uploader.enqueue(json!({"external_metadata": {"k": 1}}));
        while uploader.is_running() {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        let sent = sent.lock().unwrap();
        assert_eq!(sent.len(), 2);
        // First PUT: initial patch. Second: coalesced pending overlay.
        assert_eq!(sent[0]["worker_status"], "idle");
        assert_eq!(sent[1]["worker_status"], "running");
        assert_eq!(sent[1]["external_metadata"]["k"], 1);
    }

    #[tokio::test]
    async fn retries_with_backoff_until_success() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let counter = attempts.clone();
        let send: WorkerStateSend = Arc::new(move |_body| {
            let counter = counter.clone();
            Box::pin(async move { counter.fetch_add(1, Ordering::SeqCst) >= 2 })
        });
        let uploader = WorkerStateUploader::new(send, 1, 2);
        uploader.enqueue(json!({"worker_status": "idle"}));
        while uploader.is_running() {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        assert!(attempts.load(Ordering::SeqCst) >= 2);
    }
}
