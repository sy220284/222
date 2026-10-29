//! Backward compatible control request keys for NDJSON SDK transports.
use serde_json::{Map, Value};

pub fn normalize_control_message_keys(mut input: Value) -> Value {
    let Value::Object(ref mut record) = input else {
        return input;
    };
    rename_one(record);
    if let Some(Value::Object(ref mut response)) = record.get_mut("response") {
        rename_one(response);
    }
    input
}
fn rename_one(record: &mut Map<String, Value>) {
    if !record.contains_key("request_id") {
        if let Some(value) = record.remove("requestId") {
            record.insert("request_id".into(), value);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn nested_and_top_level_legacy_ids_are_normalized() {
        assert_eq!(
            normalize_control_message_keys(json!({
                "requestId":"outer","response":{"requestId":"inner","ok":true}
            })),
            json!({"request_id":"outer",
            "response":{"request_id":"inner","ok":true}})
        );
    }
    #[test]
    fn snakecase_wins_without_deleting_legacy_field() {
        let v = json!({"requestId":"old","request_id":"new",
            "response":{"requestId":"nested-old","request_id":"nested-new"}});
        assert_eq!(normalize_control_message_keys(v.clone()), v);
    }
    #[test]
    fn arrays_primitives_and_more_nested_objects_are_untouched() {
        for v in [
            Value::Null,
            json!(8),
            json!("str"),
            json!([{"requestId":"item"}]),
            json!({"response":[{"requestId":"item"}],
                "inner":{"requestId":"deep"}}),
        ] {
            assert_eq!(normalize_control_message_keys(v.clone()), v);
        }
    }
}
