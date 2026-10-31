//! Exact original 2.1.88 parseInt(..., 10) timeout semantics for common safe integer inputs.
use std::collections::HashMap;

fn is_js_whitespace(c: char) -> bool {
    matches!(c, '\u{0009}'..='\u{000D}' | ' ' | '\u{00A0}' | '\u{1680}'
        | '\u{2000}'..='\u{200A}' | '\u{2028}' | '\u{2029}' | '\u{202F}'
        | '\u{205F}' | '\u{3000}' | '\u{FEFF}')
}
fn decimal_prefix(raw: &str) -> Option<u64> {
    // Rust's Unicode trim_start incorrectly removes NEL (U+0085),
    // while JavaScript Number.parseInt(..., 10) does not.
    let raw = raw.trim_start_matches(is_js_whitespace);
    let digits = raw.strip_prefix('+').unwrap_or(raw);
    if digits.starts_with('-') {
        return None;
    }
    let prefix = digits
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>();
    if prefix.is_empty() {
        return None;
    }
    prefix.parse::<u64>().ok()
}
pub fn default_bash_timeout_ms(env: &HashMap<String, String>) -> u64 {
    env.get("BASH_DEFAULT_TIMEOUT_MS")
        .and_then(|value| decimal_prefix(value))
        .filter(|value| *value > 0)
        .unwrap_or(120_000)
}
pub fn max_bash_timeout_ms(env: &HashMap<String, String>) -> u64 {
    env.get("BASH_MAX_TIMEOUT_MS")
        .and_then(|value| decimal_prefix(value))
        .filter(|value| *value > 0)
        .unwrap_or(600_000)
        .max(default_bash_timeout_ms(env))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_parse_int_and_javascript_whitespace_behavior() {
        let mut env = HashMap::new();
        assert_eq!(default_bash_timeout_ms(&env), 120_000);
        assert_eq!(max_bash_timeout_ms(&env), 600_000);
        env.insert("BASH_DEFAULT_TIMEOUT_MS".into(), "\u{0085}123".into());
        assert_eq!(default_bash_timeout_ms(&env), 120_000);
        env.insert("BASH_DEFAULT_TIMEOUT_MS".into(), "\u{FEFF}123ms".into());
        assert_eq!(default_bash_timeout_ms(&env), 123);
        env.insert("BASH_MAX_TIMEOUT_MS".into(), "1e7".into());
        assert_eq!(max_bash_timeout_ms(&env), 123);
        env.insert("BASH_MAX_TIMEOUT_MS".into(), "15ms".into());
        assert_eq!(max_bash_timeout_ms(&env), 123);
    }
    #[test]
    fn max_timeout_never_below_configured_default() {
        let env = HashMap::from([
            ("BASH_DEFAULT_TIMEOUT_MS".into(), "900000".into()),
            ("BASH_MAX_TIMEOUT_MS".into(), "200000".into()),
        ]);
        assert_eq!(default_bash_timeout_ms(&env), 900000);
        assert_eq!(max_bash_timeout_ms(&env), 900000);
    }
}
