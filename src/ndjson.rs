//! A single-line JSON transport must escape JavaScript line separators.
use serde::ser::{Serialize, SerializeMap, SerializeSeq, Serializer};
use serde_json::Value;

/// JavaScript JSON.stringify enumerates canonical array-index object keys
/// first, in ascending numeric order. Other keys retain insertion order.
fn js_array_index(key: &str) -> Option<u32> {
    let n = key.parse::<u32>().ok()?;
    (n != u32::MAX && n.to_string() == key).then_some(n)
}

/// Preserve ECMA-262 property ordering when emitting nested SDK JSON values.
/// Ordinary serde_json::Value maps are insertion ordered through
/// serde_json/preserve_order; numeric property keys need a second pass.
struct JsOrdered<'a>(&'a Value);

impl Serialize for JsOrdered<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            Value::Object(map) => {
                let mut indexes = map
                    .iter()
                    .filter_map(|(key, value)| js_array_index(key).map(|index| (index, key, value)))
                    .collect::<Vec<_>>();
                indexes.sort_unstable_by_key(|(index, _, _)| *index);
                let mut output = serializer.serialize_map(Some(map.len()))?;
                for (_, key, value) in indexes {
                    output.serialize_entry(key, &JsOrdered(value))?;
                }
                for (key, value) in map {
                    if js_array_index(key).is_none() {
                        output.serialize_entry(key, &JsOrdered(value))?;
                    }
                }
                output.end()
            }
            Value::Array(items) => {
                let mut output = serializer.serialize_seq(Some(items.len()))?;
                for value in items {
                    output.serialize_element(&JsOrdered(value))?;
                }
                output.end()
            }
            Value::Number(number)
                if number.as_f64() == Some(0.0) && number.to_string().starts_with('-') =>
            {
                serializer.serialize_i32(0)
            }
            other => other.serialize(serializer),
        }
    }
}

pub fn stringify_value(value: &Value) -> String {
    serde_json::to_string(&JsOrdered(value))
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
    fn javascript_array_index_keys_precede_string_keys_in_nested_sdk_json() {
        let source: Value = serde_json::from_str(
            r#"{"z":0,"10":"ten","2":"two","01":"leading","1":"one","4294967295":"out","0":"zero","a":1}"#,
        )
        .unwrap();
        assert_eq!(
            stringify_value(&source),
            r#"{"0":"zero","1":"one","2":"two","10":"ten","z":0,"01":"leading","4294967295":"out","a":1}"#
        );
        let nested: Value =
            serde_json::from_str(r#"{"items":[{"3":"third","1":"first","x":true}],"v":-0}"#)
                .unwrap();
        assert_eq!(
            stringify_value(&nested),
            r#"{"items":[{"1":"first","3":"third","x":true}],"v":0}"#
        );
        assert_eq!(js_array_index("0"), Some(0));
        assert_eq!(js_array_index("4294967294"), Some(4294967294));
        for key in [
            "",
            "00",
            "01",
            "-1",
            "1.0",
            "+1",
            "4294967295",
            "99999999999",
        ] {
            assert_eq!(js_array_index(key), None, "invalid array-index key: {key}");
        }
    }

    #[test]
    fn escapes_multiple_separators_and_preserves_ordinary_json() {
        let original = serde_json::json!({"v":["\u{2028}\u{2028}","\u{2029}","ok"],"n":null});
        let line = stringify_value(&original);
        assert_eq!(line.matches("\\u2028").count(), 2);
        assert_eq!(serde_json::from_str::<Value>(&line).unwrap(), original);
    }
}
