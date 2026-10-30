//! Unicode hidden-prompt mitigation, ported from
//! src/utils/sanitization.ts. NFKC and Unicode Cn/Cf/Co removal are
//! deliberately iterative to catch characters exposed by normalization.
use regex::Regex;
use serde_json::{Map, Value};
use std::sync::OnceLock;
use unicode_normalization::UnicodeNormalization;

fn dangerous_unicode() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[\p{Cf}\p{Co}\p{Cn}]").expect("Unicode categories"))
}

/// Preserve readable Unicode and ordinary whitespace; erase hidden format,
/// private-use, unassigned and directional-control codepoints.
pub fn partially_sanitize_unicode(input: &str) -> String {
    let mut current = input.to_owned();
    let mut previous = String::new();
    let mut iterations = 0;
    while current != previous && iterations < 10 {
        previous = current.clone();
        let normalized: String = current.nfkc().collect();
        let stripped = dangerous_unicode().replace_all(&normalized, "");
        current = stripped
            .chars()
            .filter(|c| {
                !matches!(*c as u32,
                    0x200B..=0x200F | 0x202A..=0x202E |
                    0x2066..=0x2069 | 0xFEFF | 0xE000..=0xF8FF)
            })
            .collect();
        iterations += 1;
    }
    // NFKC reaches a fixed point. The source throws if the limit is hit;
    // an explicit check keeps a malicious input from silently bypassing it.
    assert!(
        iterations < 10,
        "Unicode sanitization exceeded 10 iterations"
    );
    current
}

/// The original recursively walks arrays and objects, sanitizing object keys
/// as well as string values and leaving other JSON primitives unchanged.
pub fn recursively_sanitize_unicode(value: Value) -> Value {
    match value {
        Value::String(s) => Value::String(partially_sanitize_unicode(&s)),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(recursively_sanitize_unicode)
                .collect(),
        ),
        Value::Object(items) => {
            let mut result = Map::new();
            for (key, val) in items {
                result.insert(
                    partially_sanitize_unicode(&key),
                    recursively_sanitize_unicode(val),
                );
            }
            Value::Object(result)
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn normalization_and_hidden_character_filtering() {
        assert_eq!(partially_sanitize_unicode("ＡＢＣ"), "ABC");
        assert_eq!(partially_sanitize_unicode("a\u{200b}b\u{202e}c"), "abc");
        assert_eq!(partially_sanitize_unicode("hi\u{e000}😀 中文"), "hi😀 中文");
        assert_eq!(partially_sanitize_unicode("a\nb"), "a\nb");
        assert_eq!(partially_sanitize_unicode("safe\u{e0001}text"), "safetext");
    }

    #[test]
    fn nested_json_keys_values_are_sanitized() {
        let input = json!({"a\u{200b}b":[{"\u{200e}key":"ｗorld"},false,42,null]});
        let result = recursively_sanitize_unicode(input);
        assert_eq!(result, json!({"ab":[{"key":"world"},false,42,null]}));
    }

    #[test]
    fn nfkc_can_expose_nested_hidden_codepoints() {
        let twice = partially_sanitize_unicode("ﬁ Ａ\u{2066}B\u{2069}");
        assert_eq!(twice, "fi AB");
    }
}
