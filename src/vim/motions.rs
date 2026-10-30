//! Vim motion resolution, ported from `src/vim/motions.ts` (2.1.88).
//! Pure functions: resolve a motion key to a target cursor without
//! modifying anything.

use super::cursor::VimCursor;

/// Resolve a motion to a target cursor position (repeat `count` times,
/// stopping early when the cursor stops moving).
pub fn resolve_motion<'a>(key: &str, cursor: &VimCursor<'a>, count: usize) -> VimCursor<'a> {
    let mut result = VimCursor::new(cursor.text, cursor.offset);
    let count = count.max(1);
    for _ in 0..count {
        let next = apply_single_motion(key, &result);
        if next.equals(&result) {
            break;
        }
        result = next;
    }
    result
}

/// Apply a single motion step.
fn apply_single_motion<'a>(key: &str, cursor: &VimCursor<'a>) -> VimCursor<'a> {
    match key {
        "h" => cursor.left(),
        "l" => cursor.right(),
        "j" => cursor.down_logical_line(),
        "k" => cursor.up_logical_line(),
        "gj" => cursor.down_logical_line(),
        "gk" => cursor.up_logical_line(),
        "w" => cursor.next_vim_word(),
        "b" => cursor.prev_vim_word(),
        "e" => cursor.end_of_vim_word(),
        "W" => cursor.next_word_big(),
        "B" => cursor.prev_word_big(),
        "E" => cursor.end_of_word_big(),
        "0" => cursor.start_of_logical_line(),
        "^" => cursor.first_non_blank_in_logical_line(),
        "$" => cursor.end_of_logical_line(),
        "G" => cursor.start_of_last_line(),
        _ => VimCursor::new(cursor.text, cursor.offset),
    }
}

/// Inclusive motions operate on the character at the destination.
pub fn is_inclusive_motion(key: &str) -> bool {
    matches!(key, "e" | "E" | "$")
}

/// Linewise motions operate on full lines with operators. Per `:help gj`,
/// gj/gk are characterwise exclusive, not linewise.
pub fn is_linewise_motion(key: &str) -> bool {
    matches!(key, "j" | "k" | "G" | "gg")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resolve_motion_applies_counts_and_stops_at_edges() {
        let text = "one two three";
        assert_eq!(resolve_motion("w", &VimCursor::new(text, 0), 2).offset, 8);
        // h at start cannot move further
        assert_eq!(resolve_motion("h", &VimCursor::new(text, 0), 5).offset, 0);
        assert_eq!(
            resolve_motion("$", &VimCursor::new(text, 0), 1).offset,
            text.len()
        );
        assert_eq!(resolve_motion("^", &VimCursor::new(text, 0), 1).offset, 0);
        // unknown keys are a no-op
        assert_eq!(resolve_motion("z", &VimCursor::new(text, 4), 3).offset, 4);
    }
    #[test]
    fn motion_classification() {
        assert!(is_inclusive_motion("e") && is_inclusive_motion("$") && !is_inclusive_motion("w"));
        assert!(is_linewise_motion("j") && is_linewise_motion("gg") && is_linewise_motion("G"));
        assert!(!is_linewise_motion("gj"));
    }
}
