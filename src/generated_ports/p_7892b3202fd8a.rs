//! Rust exports derived from original: src/components/permissions/utils.ts
//!
//! Unary permission event construction. The vendor telemetry sink is
//! excluded, so the port builds the event payload (metadata shape preserved)
//! and leaves delivery to the unary logging port (p_5386fc6ccfc1e).

use serde_json::{json, Value};

/// `logUnaryPermissionEvent` payload construction: completion type, accept or
/// reject, message id, platform and feedback flag.
pub fn unary_permission_event(
    completion_type: &str,
    message_id: &str,
    event: &str,
    platform: &str,
    has_feedback: Option<bool>,
) -> Value {
    json!({
        "completion_type": completion_type,
        "event": event,
        "metadata": {
            "language_name": "none",
            "message_id": message_id,
            "platform": platform,
            "hasFeedback": has_feedback.unwrap_or(false),
        },
    })
}
