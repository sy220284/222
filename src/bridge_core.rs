//! Local, provider-neutral bridge message sequencing. 2.1.88 reference:
//! `src/bridge/flushGate.ts`, `src/bridge/sessionIdCompat.ts`,
//! `src/bridge/pollConfigDefaults.ts`.
//!
//! The bridge's network adapter is responsible for writing drained messages;
//! the SDK control protocol exposes the same ordered handoff to that adapter.
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

/// A single session owns its pending outbound messages. No process-global queue.
#[derive(Debug)]
pub struct BridgeSession {
    session_id: String,
    active: bool,
    pending: Vec<Value>,
}

impl BridgeSession {
    pub fn new(session_id: String) -> Self {
        Self {
            session_id,
            active: false,
            pending: Vec::new(),
        }
    }

    pub fn dispatch(&mut self, action: &str, request: &Value) -> Result<Value> {
        // Never allow an SDK client to operate on a different session's buffer.
        if let Some(id) = request
            .get("session_id")
            .or_else(|| request.get("sessionId"))
        {
            let requested = id.as_str().context("bridge session_id must be a string")?;
            if requested != self.session_id {
                bail!("bridge session does not match current SDK session");
            }
        }
        match action {
            "bridge_flush_start" => {
                self.active = true;
                Ok(self.status())
            }
            "bridge_flush_enqueue" => {
                let messages = request
                    .get("messages")
                    .and_then(Value::as_array)
                    .context("bridge_flush_enqueue requires messages array")?;
                if self.active {
                    self.pending.extend(messages.iter().cloned());
                }
                Ok(json!({"queued":self.active,"pending_count":self.pending.len()}))
            }
            "bridge_flush_end" => {
                self.active = false;
                let messages = std::mem::take(&mut self.pending);
                Ok(json!({"messages":messages,"pending_count":0,"active":false}))
            }
            "bridge_flush_drop" => {
                self.active = false;
                let count = self.pending.len();
                self.pending.clear();
                Ok(json!({"dropped":count,"pending_count":0,"active":false}))
            }
            "bridge_flush_deactivate" => {
                // Transport replacement preserves the queue for the next adapter.
                self.active = false;
                Ok(self.status())
            }
            "bridge_flush_status" => Ok(self.status()),
            "bridge_session_ids" => {
                let id = request
                    .get("id")
                    .and_then(Value::as_str)
                    .context("bridge_session_ids requires id string")?;
                let compat_enabled = request
                    .get("compat_enabled")
                    .and_then(Value::as_bool)
                    .unwrap_or(true);
                Ok(json!({"compat_id":to_compat_session_id(id, compat_enabled),
                    "infra_id":to_infra_session_id(id)}))
            }
            "bridge_poll_defaults" => Ok(poll_defaults()),
            _ => bail!("Unknown bridge action: {action}"),
        }
    }

    fn status(&self) -> Value {
        json!({"active":self.active,"pending_count":self.pending.len()})
    }
}

pub fn to_compat_session_id(id: &str, enabled: bool) -> String {
    if enabled {
        id.strip_prefix("cse_")
            .map(|rest| format!("session_{rest}"))
            .unwrap_or_else(|| id.to_owned())
    } else {
        id.to_owned()
    }
}

pub fn to_infra_session_id(id: &str) -> String {
    id.strip_prefix("session_")
        .map(|rest| format!("cse_{rest}"))
        .unwrap_or_else(|| id.to_owned())
}

/// Exact source defaults, with no dependency on a specific hosted provider.
pub fn poll_defaults() -> Value {
    json!({
        "poll_interval_ms_not_at_capacity":2000,
        "poll_interval_ms_at_capacity":600000,
        "non_exclusive_heartbeat_interval_ms":0,
        "multisession_poll_interval_ms_not_at_capacity":2000,
        "multisession_poll_interval_ms_partial_capacity":2000,
        "multisession_poll_interval_ms_at_capacity":600000,
        "reclaim_older_than_ms":5000,
        "session_keepalive_interval_v2_ms":120000
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flushing_history_preserves_incoming_order_and_only_enqueues_when_active() {
        let mut bridge = BridgeSession::new("a".to_owned());
        assert_eq!(
            bridge
                .dispatch("bridge_flush_enqueue", &json!({"messages":[1]}))
                .unwrap()["queued"],
            false
        );
        bridge.dispatch("bridge_flush_start", &json!({})).unwrap();
        bridge
            .dispatch(
                "bridge_flush_enqueue",
                &json!({"messages":[{"seq":1},{"seq":2}]}),
            )
            .unwrap();
        bridge
            .dispatch("bridge_flush_enqueue", &json!({"messages":[{"seq":3}]}))
            .unwrap();
        assert_eq!(
            bridge.dispatch("bridge_flush_end", &json!({})).unwrap()["messages"],
            json!([{"seq":1},{"seq":2},{"seq":3}])
        );
        assert_eq!(
            bridge.dispatch("bridge_flush_end", &json!({})).unwrap()["messages"],
            json!([])
        );
    }

    #[test]
    fn transport_swap_keeps_unsent_messages_and_permanent_close_discards_them() {
        let mut bridge = BridgeSession::new("a".to_owned());
        bridge.dispatch("bridge_flush_start", &json!({})).unwrap();
        bridge
            .dispatch("bridge_flush_enqueue", &json!({"messages":["before"]}))
            .unwrap();
        assert_eq!(
            bridge
                .dispatch("bridge_flush_deactivate", &json!({}))
                .unwrap()["pending_count"],
            1
        );
        assert_eq!(
            bridge
                .dispatch("bridge_flush_enqueue", &json!({"messages":["direct"]}))
                .unwrap()["queued"],
            false
        );
        bridge.dispatch("bridge_flush_start", &json!({})).unwrap();
        bridge
            .dispatch("bridge_flush_enqueue", &json!({"messages":["after"]}))
            .unwrap();
        assert_eq!(
            bridge.dispatch("bridge_flush_end", &json!({})).unwrap()["messages"],
            json!(["before", "after"])
        );
        bridge.dispatch("bridge_flush_start", &json!({})).unwrap();
        bridge
            .dispatch("bridge_flush_enqueue", &json!({"messages":[5,6]}))
            .unwrap();
        assert_eq!(
            bridge.dispatch("bridge_flush_drop", &json!({})).unwrap()["dropped"],
            2
        );
        assert_eq!(
            bridge.dispatch("bridge_flush_end", &json!({})).unwrap()["messages"],
            json!([])
        );
    }

    #[test]
    fn session_owner_and_input_validation_precede_mutation() {
        let mut bridge = BridgeSession::new("local".to_owned());
        assert!(bridge
            .dispatch("bridge_flush_start", &json!({"session_id":"other"}))
            .is_err());
        assert_eq!(
            bridge.dispatch("bridge_flush_status", &json!({})).unwrap()["active"],
            false
        );
        bridge.dispatch("bridge_flush_start", &json!({})).unwrap();
        assert!(bridge
            .dispatch("bridge_flush_enqueue", &json!({"messages":"incorrect"}))
            .is_err());
        assert!(bridge
            .dispatch(
                "bridge_flush_enqueue",
                &json!({"sessionId":"other","messages":[1]})
            )
            .is_err());
        assert_eq!(
            bridge.dispatch("bridge_flush_status", &json!({})).unwrap()["pending_count"],
            0
        );
    }

    #[test]
    fn session_retag_matches_original_shim_contract() {
        assert_eq!(to_compat_session_id("cse_abc", true), "session_abc");
        assert_eq!(to_compat_session_id("cse_abc", false), "cse_abc");
        assert_eq!(to_compat_session_id("session_abc", true), "session_abc");
        assert_eq!(to_infra_session_id("session_abc"), "cse_abc");
        assert_eq!(to_infra_session_id("cse_abc"), "cse_abc");
        assert_eq!(to_compat_session_id("cse_", true), "session_");
    }

    #[test]
    fn source_poll_defaults_are_consistent_for_single_and_multiple_sessions() {
        let v = poll_defaults();
        assert_eq!(v["poll_interval_ms_not_at_capacity"], 2000);
        assert_eq!(v["multisession_poll_interval_ms_partial_capacity"], 2000);
        assert_eq!(v["poll_interval_ms_at_capacity"], 600000);
        assert_eq!(v["session_keepalive_interval_v2_ms"], 120000);
    }
}
