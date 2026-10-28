//! Equivalent to the original leaf JSON reader's stripBOM function.
//! Removes one leading U+FEFF, not all BOMs and not leading whitespace.
pub fn strip_bom(input: &str) -> &str {
    input.strip_prefix('\u{feff}').unwrap_or(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn removes_exactly_one_leading_utf8_bom() {
        assert_eq!(strip_bom("\u{feff}{\"x\":1}"), "{\"x\":1}");
        assert_eq!(strip_bom("\u{feff}\u{feff}abc"), "\u{feff}abc");
        assert_eq!(strip_bom(" \u{feff}abc"), " \u{feff}abc");
        assert_eq!(strip_bom("abc\u{feff}"), "abc\u{feff}");
        assert_eq!(strip_bom(""), "");
    }
}
