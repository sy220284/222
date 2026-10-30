//! Rust exports derived from original: src/hooks/useIdeLogging.ts
//!
//! IDE log_event bridge: validates the MCP notification against the log_event
//! schema (method literal + eventName string + passthrough eventData) and
//! forwards to the analytics port. The zod schema becomes a serde check; the
//! analytics sink is the excluded vendor telemetry surface.

/// Schema check for the `log_event` MCP notification.
pub fn is_log_event_notification(method: &str, body: &serde_json::Value) -> bool {
    method == "log_event"
        && body
            .get("eventName")
            .and_then(serde_json::Value::as_str)
            .is_some()
        && body
            .get("eventData")
            .map(serde_json::Value::is_object)
            .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn schema_gate() {
        assert!(is_log_event_notification(
            "log_event",
            &json!({"eventName":"x","eventData":{}})
        ));
        assert!(!is_log_event_notification("other", &json!({})));
        assert!(!is_log_event_notification(
            "log_event",
            &json!({"eventName":"x"})
        ));
    }
}
