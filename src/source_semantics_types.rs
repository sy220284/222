//! Pure-state terminal and editor helpers from original 2.1.88 source.
#![allow(dead_code)]
use serde_json::{json, Value};

/// src/ink/termio/types.ts: reset SGR state.
pub fn default_style() -> Value {
    json!({
        "bold": false,
        "dim": false,
        "italic": false,
        "underline": "none",
        "blink": false,
        "inverse": false,
        "hidden": false,
        "strikethrough": false,
        "overline": false,
        "fg": {"type":"default"},
        "bg": {"type":"default"},
        "underlineColor": {"type":"default"}
    })
}
/// Compare each tagged variant by the exact source-defined fields,
/// not arbitrary extra metadata from downstream renderers.
pub fn colors_equal(a: &Value, b: &Value) -> bool {
    let ty = a.get("type").and_then(Value::as_str);
    if ty != b.get("type").and_then(Value::as_str) {
        return false;
    }
    match ty {
        Some("named") => a.get("name") == b.get("name"),
        Some("indexed") => a.get("index") == b.get("index"),
        Some("rgb") => ["r", "g", "b"].iter().all(|key| a.get(key) == b.get(key)),
        Some("default") => true,
        _ => false,
    }
}
pub fn styles_equal(a: &Value, b: &Value) -> bool {
    for key in [
        "bold",
        "dim",
        "italic",
        "underline",
        "blink",
        "inverse",
        "hidden",
        "strikethrough",
        "overline",
    ] {
        if a.get(key) != b.get(key) {
            return false;
        }
    }
    ["fg", "bg", "underlineColor"].iter().all(|key| {
        a.get(key)
            .zip(b.get(key))
            .is_some_and(|(left, right)| colors_equal(left, right))
    })
}
/// src/vim/motions.ts: exactly the original inclusive and linewise sets.
/// In JavaScript includes("") is true, so the empty-key boundary is preserved.
pub fn is_inclusive_motion(key: &str) -> bool {
    "eE$".contains(key)
}
pub fn is_linewise_motion(key: &str) -> bool {
    "jkG".contains(key) || key == "gg"
}
pub fn inspect(source: &str, export: &str, args: &[Value]) -> Option<Value> {
    let a = args.first().unwrap_or(&Value::Null);
    let b = args.get(1).unwrap_or(&Value::Null);
    match (source, export) {
        ("src/ink/termio/types.ts", "defaultStyle") => Some(default_style()),
        ("src/ink/termio/types.ts", "colorsEqual") => Some(json!(colors_equal(a, b))),
        ("src/ink/termio/types.ts", "stylesEqual") => Some(json!(styles_equal(a, b))),
        ("src/vim/motions.ts", "isInclusiveMotion") => {
            Some(json!(is_inclusive_motion(a.as_str().unwrap_or(""))))
        }
        ("src/vim/motions.ts", "isLinewiseMotion") => {
            Some(json!(is_linewise_motion(a.as_str().unwrap_or(""))))
        }
        _ => None,
    }
}
pub fn exports(source: &str) -> Option<&'static [&'static str]> {
    match source {
        "src/ink/termio/types.ts" => Some(&["defaultStyle", "colorsEqual", "stylesEqual"]),
        "src/vim/motions.ts" => Some(&["isInclusiveMotion", "isLinewiseMotion"]),
        _ => None,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tagged_color_identity_and_style_transitions() {
        let base = default_style();
        assert!(styles_equal(&base, &base));
        assert!(colors_equal(
            &json!({"type":"default"}),
            &json!({"type":"default","metadata":1})
        ));
        assert!(!colors_equal(
            &json!({"type":"rgb","r":1,"g":2,"b":3}),
            &json!({"type":"rgb","r":1,"g":3,"b":3})
        ));
        let mut bold = base.clone();
        bold["bold"] = json!(true);
        assert!(!styles_equal(&base, &bold));
        let mut foreground = base.clone();
        foreground["fg"] = json!({"type":"named","name":"red"});
        assert!(!styles_equal(&base, &foreground));
    }
    #[test]
    fn vim_motion_flags_use_original_key_semantics() {
        assert!(is_inclusive_motion("e"));
        assert!(is_inclusive_motion("$"));
        assert!(!is_inclusive_motion("w"));
        assert!(is_linewise_motion("j"));
        assert!(is_linewise_motion("gg"));
        assert!(!is_linewise_motion("gj"));
        assert!(is_linewise_motion(""));
    }
}
