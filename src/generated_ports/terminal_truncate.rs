//! Manual Rust implementation of `src/utils/truncate.ts` text-width helpers.
//! Uses the project's terminal width model and keeps combining/ZWJ/emoji
//! sequences intact during truncation. The host supplies no JS runtime.
#![allow(dead_code)]
use crate::portable_ink_core::LineWidthCache;
use serde_json::{json, Value};
use unicode_normalization::char::is_combining_mark;

fn regional_indicator(ch: char) -> bool {
    ('\u{1f1e6}'..='\u{1f1ff}').contains(&ch)
}
fn join_next(ch: char, prev: char, regionals: usize) -> bool {
    is_combining_mark(ch)
        || ch == '\u{200d}'
        || prev == '\u{200d}'
        || ('\u{fe00}'..='\u{fe0f}').contains(&ch)
        || ('\u{1f3fb}'..='\u{1f3ff}').contains(&ch)
        || (regional_indicator(ch) && regional_indicator(prev) && regionals % 2 == 1)
}
fn clusters(s: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut last = '\0';
    let mut regional_count = 0usize;
    let mut pending_ansi = String::new();
    let mut chars = s.char_indices().peekable();
    while let Some((offset, ch)) = chars.next() {
        // Keep terminal CSI and OSC control sequences intact. Each escape
        // sequence has zero display cells, and belongs to the following
        // grapheme so truncation never breaks in the middle of a code.
        if ch == '\u{1b}' {
            let suffix = &s[offset..];
            let sequence_end = if suffix.starts_with("\u{1b}[") {
                suffix[2..]
                    .char_indices()
                    .find(|&(_, c)| ('@'..='~').contains(&c))
                    .map(|(i, c)| 2 + i + c.len_utf8())
            } else if suffix.starts_with("\u{1b}]") {
                suffix[2..]
                    .char_indices()
                    .find(|&(_, c)| c == '\u{7}')
                    .map(|(i, c)| 2 + i + c.len_utf8())
            } else {
                None
            };
            if let Some(end) = sequence_end {
                let byte_end = offset + end;
                pending_ansi.push_str(&s[offset..byte_end]);
                while chars.peek().is_some_and(|(next, _)| *next < byte_end) {
                    chars.next();
                }
                continue;
            }
        }
        if join_next(ch, last, regional_count) && !out.is_empty() {
            let current = out.last_mut().expect("checked nonempty");
            current.push_str(&pending_ansi);
            current.push(ch);
        } else {
            out.push(format!("{pending_ansi}{ch}"));
        }
        pending_ansi.clear();
        if regional_indicator(ch) {
            regional_count += 1;
        } else {
            regional_count = 0;
        }
        last = ch;
    }
    if !pending_ansi.is_empty() {
        if let Some(last) = out.last_mut() {
            last.push_str(&pending_ansi);
        } else {
            out.push(pending_ansi);
        }
    }
    out
}
pub fn display_width(s: &str) -> usize {
    let mut cache = LineWidthCache::default();
    clusters(s)
        .into_iter()
        .map(|grapheme| {
            let measured = cache.line_width(&grapheme);
            if grapheme.contains('\u{200d}') {
                2
            } else if grapheme.chars().filter(|&c| regional_indicator(c)).count() == 2 {
                2
            } else {
                measured
            }
        })
        .sum()
}
fn width(s: &str) -> usize {
    display_width(s)
}
pub fn truncate_to_width(text: &str, max_width: isize) -> String {
    if width(text) as isize <= max_width {
        return text.into();
    }
    if max_width <= 1 {
        return "…".into();
    }
    let mut result = String::new();
    let mut total = 0usize;
    for g in clusters(text) {
        let w = width(&g);
        if total + w > (max_width - 1) as usize {
            break;
        }
        result.push_str(&g);
        total += w;
    }
    result.push('…');
    result
}
pub fn truncate_start_to_width(text: &str, max_width: isize) -> String {
    if width(text) as isize <= max_width {
        return text.into();
    }
    if max_width <= 1 {
        return "…".into();
    }
    let graphemes = clusters(text);
    let mut chosen = Vec::new();
    let mut total = 0;
    for g in graphemes.into_iter().rev() {
        let w = width(&g);
        if total + w > (max_width - 1) as usize {
            break;
        }
        total += w;
        chosen.push(g);
    }
    chosen.reverse();
    format!("…{}", chosen.concat())
}
pub fn truncate_to_width_no_ellipsis(text: &str, max_width: isize) -> String {
    if width(text) as isize <= max_width {
        return text.into();
    }
    if max_width <= 0 {
        return String::new();
    }
    let mut result = String::new();
    let mut total = 0;
    for g in clusters(text) {
        let w = width(&g);
        if total + w > max_width as usize {
            break;
        }
        result.push_str(&g);
        total += w;
    }
    result
}
pub fn truncate_path_middle(path: &str, max_length: isize) -> String {
    if width(path) as isize <= max_length {
        return path.into();
    }
    if max_length <= 0 {
        return "…".into();
    }
    if max_length < 5 {
        return truncate_to_width(path, max_length);
    }
    let (dir, file) = path
        .rfind('/')
        .map_or(("", path), |index| path.split_at(index));
    let file_width = width(file) as isize;
    if file_width >= max_length - 1 {
        return truncate_start_to_width(path, max_length);
    }
    let available = max_length - 1 - file_width;
    if available <= 0 {
        return truncate_start_to_width(file, max_length);
    }
    format!("{}…{}", truncate_to_width_no_ellipsis(dir, available), file)
}
pub fn truncate(text: &str, max_width: isize, single_line: bool) -> String {
    if single_line {
        if let Some((first, _)) = text.split_once('\n') {
            if width(first) as isize + 1 > max_width {
                return truncate_to_width(first, max_width);
            }
            return format!("{first}…");
        }
    }
    truncate_to_width(text, max_width)
}
pub fn wrap_text(text: &str, max_width: isize) -> Vec<String> {
    let mut lines = Vec::<String>::new();
    let mut current = String::new();
    let mut total = 0usize;
    for g in clusters(text) {
        let w = width(&g);
        if total as isize + w as isize <= max_width {
            current.push_str(&g);
            total += w;
        } else {
            if !current.is_empty() {
                lines.push(current);
            }
            current = g;
            total = w;
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

pub fn inspect(export: &str, args: &[Value]) -> Option<Value> {
    let text = args.first()?.as_str()?;
    let n = args.get(1)?.as_i64()? as isize;
    Some(match export {
        "truncatePathMiddle" => json!(truncate_path_middle(text, n)),
        "truncateToWidth" => json!(truncate_to_width(text, n)),
        "truncateStartToWidth" => json!(truncate_start_to_width(text, n)),
        "truncateToWidthNoEllipsis" => json!(truncate_to_width_no_ellipsis(text, n)),
        "truncate" => json!(truncate(
            text,
            n,
            args.get(2).and_then(Value::as_bool).unwrap_or(false)
        )),
        "wrapText" => json!(wrap_text(text, n)),
        _ => return None,
    })
}

#[cfg(test)]
mod parity_tests {
    use super::*;
    #[test]
    fn path_truncation_preserves_filename_and_directory_context() {
        assert_eq!(
            truncate_path_middle("src/components/MyComponent.tsx", 22),
            "src/c…/MyComponent.tsx"
        );
        assert_eq!(truncate_path_middle("abcd", 0), "…");
        assert_eq!(truncate_to_width("abcdef", 4), "abc…");
        assert_eq!(truncate_start_to_width("abcdef", 4), "…def");
        assert_eq!(truncate_to_width_no_ellipsis("abcdef", 3), "abc");
    }
    #[test]
    fn combined_unicode_graphemes_are_not_split_and_widths_are_correct() {
        assert_eq!(clusters("e\u{301}x"), vec!["e\u{301}", "x"]);
        assert_eq!(clusters("\u{301}a"), vec!["\u{301}", "a"]);
        assert_eq!(
            clusters("\u{1b}[31me\u{301}\u{1b}[0mx"),
            vec!["\u{1b}[31me\u{301}", "\u{1b}[0mx"]
        );
        assert_eq!(display_width("\u{1b}[31m你好\u{1b}[0m"), 4);
        assert_eq!(width("你好"), 4);
        assert_eq!(width("👨‍👩‍👧‍👦"), 2);
        assert_eq!(truncate_to_width("你好X", 4), "你…");
        assert_eq!(truncate_to_width("👨‍👩‍👧‍👦X", 3), "👨‍👩‍👧‍👦X");
    }
    #[test]
    fn single_line_ellipsis_and_wrapping_follow_source() {
        assert_eq!(truncate("one\ntwo", 10, true), "one…");
        assert_eq!(truncate("one\ntwo", 3, true), "one");
        assert_eq!(wrap_text("abcdef", 2), vec!["ab", "cd", "ef"]);
        assert!(wrap_text("", 2).is_empty());
    }
}
