//! Rust exports derived from original: src/ink/wrap-text.ts
//!
//! Full equivalence at the width math layer: `columns < 1` yields empty,
//! `columns == 1` yields the ellipsis, text at or under the cap passes
//! through, and start/middle/end truncation with the `…` marker follows the
//! original offsets. ANSI-aware slicing (`sliceFit` wide-char retry) rides
//! the sliceAnsi/stringWidth ports; this module carries the plain-text math
//! and the truncation positions.

pub const ELLIPSIS: &str = "…";

/// Plain-text truncation positions (ANSI-free fast path; ANSI slicing rides
/// the sliceAnsi port with one tighter-bound retry).
pub fn truncate_plain(text: &str, columns: usize, position: &str) -> String {
    if columns < 1 {
        return String::new();
    }
    if columns == 1 {
        return ELLIPSIS.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= columns {
        return text.to_owned();
    }
    match position {
        "start" => {
            let start = chars.len() - columns + 1;
            format!("{ELLIPSIS}{}", chars[start..].iter().collect::<String>())
        }
        "middle" => {
            let half = columns / 2;
            format!(
                "{}{ELLIPSIS}{}",
                chars[..half].iter().collect::<String>(),
                chars[chars.len() - (columns - half) + 1..]
                    .iter()
                    .collect::<String>()
            )
        }
        _ => format!(
            "{}{ELLIPSIS}",
            chars[..columns - 1].iter().collect::<String>()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_and_caps() {
        assert_eq!(truncate_plain("hello", 0, "end"), "");
        assert_eq!(truncate_plain("hello", 1, "end"), "…");
        assert_eq!(truncate_plain("hi", 5, "end"), "hi");
        assert_eq!(truncate_plain("hello", 4, "end"), "hel…");
        assert_eq!(truncate_plain("hello", 4, "start"), "…llo");
    }
}
