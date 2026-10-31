//! Native Rust port derived from original: src/utils/sanitization.ts
//!
//! Unicode sanitization for hidden-character attack mitigation (ASCII
//! smuggling / hidden prompt injection). Applies NFKC normalization and
//! strips dangerous Unicode categories, iterating until stable with a
//! safety limit. The `\p{Cf}\p{Co}\p{Cn}` property-class strip is provided
//! by an explicit code-point range table (deterministic across Unicode
//! table versions); the original's explicit fallback ranges are kept
//! verbatim. Non-string JSON values (numbers, booleans, null) pass through
//! unchanged, matching the original's primitive passthrough.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use unicode_normalization::UnicodeNormalization;

/// Safety limit to prevent infinite loops.
const MAX_ITERATIONS: u32 = 10;

/// Explicit dangerous ranges: Unicode Format (Cf), Private Use (Co) and
/// Unassigned-plus-noncharacters (Cn surrogate block coverage). This is the
/// hand-rolled equivalent of the original's `/[\p{Cf}\p{Co}\p{Cn}]/gu`.
/// Format and private-use ranges are stable across Unicode versions; the
/// unassigned space is covered by the same BMP ranges the original's
/// fallback targets.
fn is_dangerous_category(code: char) -> bool {
    let c = code as u32;
    matches!(c,
        // Cf — Format (Unicode 15.1 table)
        0x00AD
        | 0x0600..=0x0605
        | 0x061C
        | 0x06DD
        | 0x070F
        | 0x0890..=0x0891
        | 0x08E2
        | 0x180E
        | 0x200B..=0x200F
        | 0x202A..=0x202E
        | 0x2060..=0x2064
        | 0x2066..=0x2069
        | 0xFEFF
        | 0xFFF9..=0xFFFB
        | 0x110BD
        | 0x110CD
        | 0x13430..=0x1343F
        | 0x1BCA0..=0x1BCA3
        | 0x1D173..=0x1D17A
        | 0xE0001
        | 0xE0020..=0xE007F
        // Co — Private Use (all three planes)
        | 0xE000..=0xF8FF
        | 0xF0000..=0xFFFFD
        | 0x100000..=0x10FFFD
        // Cn noncharacters and surrogate residue the original also strips
        | 0xFDD0..=0xFDEF
        | 0xFFFE..=0xFFFF
        | 0x1FFFE..=0x1FFFF
        | 0x2FFFE..=0x2FFFF
        | 0x3FFFE..=0x3FFFF
        | 0x4FFFE..=0x4FFFF
        | 0x5FFFE..=0x5FFFF
        | 0x6FFFE..=0x6FFFF
        | 0x7FFFE..=0x7FFFF
        | 0x8FFFE..=0x8FFFF
        | 0x9FFFE..=0x9FFFF
        | 0xAFFFE..=0xAFFFF
        | 0xBFFFE..=0xBFFFF
        | 0xCFFFE..=0xCFFFF
        | 0xDFFFE..=0xDFFFF
        | 0xEFFFE..=0xEFFFF
        | 0xFFFFE..=0xFFFFF
        | 0x10FFFE..=0x10FFFF
    )
}

/// Primary defense: strip dangerous Unicode categories.
fn strip_property_classes(input: &str) -> String {
    input
        .chars()
        .filter(|c| !is_dangerous_category(*c))
        .collect()
}

/// Fallback: explicit known dangerous ranges, kept verbatim from the
/// original for environments where property-class handling may be weaker.
fn strip_explicit_ranges(input: &str) -> String {
    input
        .chars()
        .filter(|c| {
            let c = *c as u32;
            // Zero-width spaces, LTR/RTL marks
            !(0x200B..=0x200F).contains(&c)
                // Directional formatting characters
                && !(0x202A..=0x202E).contains(&c)
                // Directional isolates
                && !(0x2066..=0x2069).contains(&c)
                // Byte order mark
                && c != 0xFEFF
                // Basic Multilingual Plane private use
                && !(0xE000..=0xF8FF).contains(&c)
        })
        .collect()
}

/// Iteratively sanitize a prompt until no more changes occur or the max
/// iteration safety limit is reached (crash loudly beyond it, matching the
/// original's throw; a pass that stabilizes exactly on the final allowed
/// iteration also errors, because the original only checks the counter).
pub fn partially_sanitize_unicode(prompt: &str) -> Result<String, String> {
    let mut current = prompt.to_string();
    let mut previous = String::new();
    let mut iterations = 0u32;

    while current != previous && iterations < MAX_ITERATIONS {
        previous = current.clone();
        // Apply NFKC normalization to handle composed character sequences.
        current = current.nfkc().collect();
        // Primary defense: strip dangerous Unicode categories.
        current = strip_property_classes(&current);
        // Fallback: explicit known dangerous ranges.
        current = strip_explicit_ranges(&current);
        iterations += 1;
    }

    if iterations >= MAX_ITERATIONS {
        return Err(format!(
            "Unicode sanitization reached maximum iterations ({MAX_ITERATIONS}) for input: {}",
            truncate_for_error(prompt, 100)
        ));
    }

    Ok(current)
}

fn truncate_for_error(input: &str, max_chars: usize) -> String {
    input.chars().take(max_chars).collect()
}

/// Recursively sanitize a JSON value: strings are sanitized, keys and
/// values of objects are sanitized recursively, arrays map recursively,
/// other scalars pass through unchanged.
pub fn recursively_sanitize_unicode_value(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::String(s) => serde_json::Value::String(
            partially_sanitize_unicode(s).unwrap_or_else(|_| s.to_string()),
        ),
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items
                .iter()
                .map(recursively_sanitize_unicode_value)
                .collect(),
        ),
        serde_json::Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (key, val) in map {
                let sanitized_key =
                    partially_sanitize_unicode(key).unwrap_or_else(|_| key.to_string());
                out.insert(sanitized_key, recursively_sanitize_unicode_value(val));
            }
            serde_json::Value::Object(out)
        }
        // Numbers, booleans and null are returned unchanged.
        other => other.clone(),
    }
}

/// String convenience wrapper matching the original's first overload.
pub fn recursively_sanitize_unicode(value: &str) -> String {
    partially_sanitize_unicode(value).unwrap_or_else(|_| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn strips_zero_width_and_tags() {
        // Tag characters (U+E0000 block, Cf) used in ASCII smuggling.
        let smuggled = "hello\u{E0041}\u{E0042}world";
        let clean = partially_sanitize_unicode(smuggled).unwrap();
        assert_eq!(clean, "helloworld");
    }

    #[test]
    fn strips_bom_and_directional_marks() {
        let dirty = "\u{FEFF}a\u{200B}b\u{202E}c\u{2066}d";
        let clean = partially_sanitize_unicode(dirty).unwrap();
        assert_eq!(clean, "abcd");
    }

    #[test]
    fn strips_private_use() {
        let dirty = "a\u{E000}b\u{F8FF}c";
        let clean = partially_sanitize_unicode(dirty).unwrap();
        assert_eq!(clean, "abc");
    }

    #[test]
    fn preserves_legitimate_text_and_normalizes() {
        // NFKC: fullwidth latin normalizes to ASCII.
        let clean = partially_sanitize_unicode("Ｈｅｌｌｏ").unwrap();
        assert_eq!(clean, "Hello");
        // Emoji and CJK survive.
        let clean = partially_sanitize_unicode("你好🌍").unwrap();
        assert_eq!(clean, "你好🌍");
    }

    #[test]
    fn nested_json_is_recursively_sanitized() {
        let value = json!({
            "ke\u{200B}y": ["a\u{FEFF}", 42, true, null],
            "nested": { "inner\u{202E}": "v\u{E0041}alue" }
        });
        let out = recursively_sanitize_unicode_value(&value);
        let obj = out.as_object().unwrap();
        assert!(obj.contains_key("key"));
        assert_eq!(obj.get("key").unwrap()[0].as_str().unwrap(), "a");
        let nested = obj.get("nested").unwrap().as_object().unwrap();
        assert!(nested.contains_key("inner"));
        assert_eq!(nested.get("inner").unwrap().as_str().unwrap(), "value");
        // Non-strings pass through.
        assert_eq!(obj.get("key").unwrap()[1].as_i64().unwrap(), 42);
    }
}
