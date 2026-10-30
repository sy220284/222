//! A single-line JSON transport must escape JavaScript line separators.
use serde_json::Value;

pub fn stringify_value(value: &Value) -> String {
    serde_json::to_string(value)
        .expect("serde_json::Value serializes")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn escape_js_line_breaks_without_changing_json_semantics() {
        let original = json!({"message":"alpha\u{2028}beta\u{2029}gamma","line":"one\ntwo"});
        let line = stringify_value(&original);
        assert!(!line.contains('\u{2028}'));
        assert!(!line.contains('\u{2029}'));
        assert!(line.contains("\\u2028"));
        assert!(line.contains("\\u2029"));
        assert!(!line.contains('\n'));
        assert_eq!(serde_json::from_str::<Value>(&line).unwrap(), original);
    }
    #[test]
    fn sdk_wire_retains_javascript_object_insertion_order_and_literal_escapes() {
        let input: Value =
            serde_json::from_str(r#"{"z":1,"a":"\u2028","\u2029field":"value","m":"\\u2028"}"#)
                .unwrap();
        let line = stringify_value(&input);
        assert!(!line.contains('\u{2028}'));
        assert!(!line.contains('\u{2029}'));
        assert_eq!(line.matches("\\u2028").count(), 2);
        assert_eq!(line.matches("\\u2029").count(), 1);
        assert!(line.find("\"z\"").unwrap() < line.find("\"a\"").unwrap());
        assert!(line.find("\"a\"").unwrap() < line.find("\\u2029field").unwrap());
        assert!(line.find("\\u2029field").unwrap() < line.find("\"m\"").unwrap());
        assert_eq!(serde_json::from_str::<Value>(&line).unwrap(), input);
    }

    #[test]
    fn escapes_multiple_separators_and_preserves_ordinary_json() {
        let original = serde_json::json!({"v":["\u{2028}\u{2028}","\u{2029}","ok"],"n":null});
        let line = stringify_value(&original);
        assert_eq!(line.matches("\\u2028").count(), 2);
        assert_eq!(serde_json::from_str::<Value>(&line).unwrap(), original);
    }
}
