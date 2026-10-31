//! Original src/utils/messagePredicates.ts human turn semantics.
use serde_json::Value;
fn js_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_f64().is_some_and(|v| v != 0.0 && !v.is_nan()),
        Value::String(text) => !text.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}
pub fn is_human_turn(message: &Value) -> bool {
    message.get("type").and_then(Value::as_str) == Some("user")
        && !message.get("isMeta").is_some_and(js_truthy)
        && message.get("toolUseResult").is_none()
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn human_messages_must_exclude_meta_truthiness_and_any_tool_result() {
        assert!(is_human_turn(&json!({"type":"user"})));
        assert!(!is_human_turn(&json!({"type":"user","isMeta":[]})));
        assert!(!is_human_turn(&json!({"type":"user","isMeta":{}})));
        assert!(!is_human_turn(&json!({"type":"user","toolUseResult":null})));
        assert!(is_human_turn(&json!({"type":"user","isMeta":0})));
    }
}
