//! Original macOS Option+key lookup, including JS's inherited "in" keys.
pub fn macos_option_char(value: &str) -> Option<&'static str> {
    match value {
        "†" => Some("alt+t"),
        "π" => Some("alt+p"),
        "ø" => Some("alt+o"),
        _ => None,
    }
}
pub fn is_macos_option_char(value: &str) -> bool {
    if macos_option_char(value).is_some() {
        return true;
    }
    // Original is: char in MACOS_OPTION_SPECIAL_CHARS, where the table
    // is an ordinary JS object. These inherited Object.prototype properties
    // are observable even though they have no shortcut equivalent.
    matches!(
        value,
        "constructor"
            | "toString"
            | "toLocaleString"
            | "valueOf"
            | "hasOwnProperty"
            | "isPrototypeOf"
            | "propertyIsEnumerable"
            | "__proto__"
            | "__defineGetter__"
            | "__defineSetter__"
            | "__lookupGetter__"
            | "__lookupSetter__"
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inherited_javascript_keys_are_true_without_shortcut_mapping() {
        assert!(is_macos_option_char("†"));
        assert_eq!(macos_option_char("†"), Some("alt+t"));
        assert!(is_macos_option_char("toString"));
        assert!(is_macos_option_char("__proto__"));
        assert_eq!(macos_option_char("toString"), None);
        assert!(!is_macos_option_char("Π"));
        assert!(!is_macos_option_char(""));
    }
}
