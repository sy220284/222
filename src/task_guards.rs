//! JSON counterpart of a pure runtime type predicate for persisted shell tasks.
use serde_json::Value;

/// Object (not null and not array) with a literal "type": "local_bash".
pub fn is_local_shell_task(task: &Value) -> bool {
    task.as_object()
        .is_some_and(|object| object.get("type").and_then(Value::as_str) == Some("local_bash"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn reference_type_guard_matrix() {
        for good in [
            json!({"type":"local_bash"}),
            json!({"type":"local_bash","status":"running"}),
            json!({"type":"local_bash","command":null}),
        ] {
            assert!(is_local_shell_task(&good));
        }
        for bad in [
            Value::Null,
            json!(false),
            json!(1),
            json!("local_bash"),
            json!([]),
            json!([{"type":"local_bash"}]),
            json!({}),
            json!({"type":null}),
            json!({"type":3}),
            json!({"type":"bash"}),
            json!({"type":"LOCAL_BASH"}),
            json!({"type":["local_bash"]}),
        ] {
            assert!(!is_local_shell_task(&bad), "{bad}");
        }
    }
}
