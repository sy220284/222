//! Original 2.1.88 user-prompt keyword detection, including ASCII JS word boundaries.
use regex::Regex;
use std::sync::OnceLock;

fn matches_js_word_boundaries(input: &str, pattern: &Regex) -> bool {
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    pattern.find_iter(input).any(|found| {
        let before = input[..found.start()].chars().next_back();
        let after = input[found.end()..].chars().next();
        !before.is_some_and(word) && !after.is_some_and(word)
    })
}
fn is_js_trim_char(c: char) -> bool {
    matches!(c, '\u{0009}'..='\u{000D}' | ' ' | '\u{00A0}' | '\u{1680}'
        | '\u{2000}'..='\u{200A}' | '\u{2028}' | '\u{2029}'
        | '\u{202F}' | '\u{205F}' | '\u{3000}' | '\u{FEFF}')
}
pub fn matches_negative_keyword(input: &str) -> bool {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    let pattern = PATTERN.get_or_init(|| {
        Regex::new(r"(wtf|wth|ffs|omfg|shit(ty|tiest)?|dumbass|horrible|awful|piss(ed|ing)? off|piece of (shit|crap|junk)|what the (fuck|hell)|fucking? (broken|useless|terrible|awful|horrible)|fuck you|screw (this|you)|so frustrating|this sucks|damn it)")
            .expect("fixed source negative keyword regex")
    });
    matches_js_word_boundaries(&input.to_lowercase(), pattern)
}
pub fn matches_keep_going_keyword(input: &str) -> bool {
    let lower = input.to_lowercase();
    let trimmed = lower.trim_matches(is_js_trim_char);
    if trimmed == "continue" {
        return true;
    }
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    let pattern = PATTERN.get_or_init(|| {
        Regex::new(r"(keep going|go on)").expect("fixed source continuation keyword regex")
    });
    matches_js_word_boundaries(trimmed, pattern)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_js_trim_preserves_nel_while_trimming_bom() {
        assert!(!matches_keep_going_keyword("\u{0085}continue"));
        assert!(matches_keep_going_keyword("\u{FEFF}continue"));
        assert!(matches_keep_going_keyword(" CONTINUE\n"));
    }
    #[test]
    fn negative_and_continuation_ascii_word_boundaries_in_unicode() {
        assert!(matches_negative_keyword("中文wtf中文"));
        assert!(matches_keep_going_keyword("中文go on中文"));
        assert!(!matches_negative_keyword("abcwtf"));
        assert!(!matches_keep_going_keyword("keep goingx"));
    }
}
