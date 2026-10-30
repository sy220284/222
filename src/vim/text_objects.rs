//! Vim text object discovery, ported from `src/vim/textObjects.ts` (2.1.88).

use super::cursor::{is_vim_punctuation, is_vim_whitespace, is_vim_word_char, VimCursor};
use super::types::TextObjScope;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextObjectRange {
    pub start: usize,
    pub end: usize,
}

/// Find the text object at the cursor: word/WORD, quotes, or bracket pairs.
/// Returns `None` when the cursor is not on a valid object.
pub fn find_text_object(
    text: &str,
    offset: usize,
    obj_type: char,
    scope: TextObjScope,
) -> Option<TextObjectRange> {
    let is_inner = scope == TextObjScope::Inner;
    let offset = offset.min(text.len());
    match obj_type {
        'w' | 'W' => {
            if obj_type == 'w' {
                find_word_object(text, offset, is_inner)
            } else {
                find_word_big_object(text, offset, is_inner)
            }
        }
        '"' | '\'' | '`' => find_quote_object(text, offset, obj_type, is_inner),
        '(' | ')' | 'b' => find_bracket_object(text, offset, '(', ')', is_inner),
        '[' | ']' => find_bracket_object(text, offset, '[', ']', is_inner),
        '{' | '}' | 'B' => find_bracket_object(text, offset, '{', '}', is_inner),
        '<' | '>' => find_bracket_object(text, offset, '<', '>', is_inner),
        _ => None,
    }
}

/// Shared helper: `aw` trailing-whitespace extension (or leading at line start).
fn extend_around(text: &str, start: &mut usize, end: &mut usize, is_ws: impl Fn(char) -> bool) {
    while *end < text.len() {
        let next = match text[*end..].chars().next() {
            Some(c) if is_ws(c) => c,
            _ => break,
        };
        *end += next.len_utf8();
    }
    if *end >= text.len() {
        while *start > 0 {
            let prev = match text[..*start].chars().next_back() {
                Some(c) if is_ws(c) => c,
                _ => break,
            };
            *start -= prev.len_utf8();
        }
    }
}

fn find_word_object(text: &str, offset: usize, is_inner: bool) -> Option<TextObjectRange> {
    let char_at = |i: usize| text[i..].chars().next();
    let c = char_at(offset)?;
    let word_start: Box<dyn Fn(char) -> bool> = if is_vim_word_char(c) {
        Box::new(is_vim_word_char)
    } else if is_vim_whitespace(c) {
        Box::new(is_vim_whitespace)
    } else {
        Box::new(is_vim_punctuation)
    };
    let mut start = offset;
    let mut end = offset;
    while start > 0 {
        let prev = text[..start].chars().next_back()?;
        if !word_start(prev) {
            break;
        }
        start -= prev.len_utf8();
    }
    while end < text.len() {
        let next = char_at(end)?;
        if !word_start(next) {
            break;
        }
        end += next.len_utf8();
    }
    if start == end {
        return None;
    }
    if !is_inner {
        // `aw` includes trailing whitespace, or leading when at line start.
        extend_around(text, &mut start, &mut end, is_vim_whitespace);
    }
    Some(TextObjectRange { start, end })
}

fn find_word_big_object(text: &str, offset: usize, is_inner: bool) -> Option<TextObjectRange> {
    let is_ws = |c: char| is_vim_whitespace(c);
    let is_non_ws = |c: char| !is_vim_whitespace(c);
    let char_at = |i: usize| text[i..].chars().next();
    let c = char_at(offset)?;
    let same: Box<dyn Fn(char) -> bool> = if is_ws(c) {
        Box::new(is_ws)
    } else {
        Box::new(is_non_ws)
    };
    let mut start = offset;
    let mut end = offset;
    while start > 0 {
        let prev = text[..start].chars().next_back()?;
        if !same(prev) {
            break;
        }
        start -= prev.len_utf8();
    }
    while end < text.len() {
        let next = char_at(end)?;
        if !same(next) {
            break;
        }
        end += next.len_utf8();
    }
    if start == end {
        return None;
    }
    if !is_inner && !is_ws(c) {
        extend_around(text, &mut start, &mut end, is_ws);
    }
    Some(TextObjectRange { start, end })
}

/// Quote objects are line-scoped: quotes pair as 0-1, 2-3, ... within the
/// line; the cursor must sit inside a pair.
fn find_quote_object(
    text: &str,
    offset: usize,
    quote: char,
    is_inner: bool,
) -> Option<TextObjectRange> {
    let line_start = text[..offset].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line_end = text[offset..]
        .find('\n')
        .map(|i| offset + i)
        .unwrap_or(text.len());
    let line_text = &text[line_start..line_end];
    let pos_in_line = offset - line_start;
    let mut positions = Vec::new();
    for (i, c) in line_text.char_indices() {
        if c == quote {
            positions.push((i, i + c.len_utf8()));
        }
    }
    for i in 0..positions.len() {
        if i + 1 >= positions.len() {
            break;
        }
        let (qs, _) = positions[i];
        let (qe, qe_next) = positions[i + 1];
        if qs <= pos_in_line && pos_in_line <= qe_next {
            return Some(if is_inner {
                TextObjectRange {
                    start: line_start + qs + 1,
                    end: line_start + qe,
                }
            } else {
                TextObjectRange {
                    start: line_start + qs,
                    end: line_start + qe_next,
                }
            });
        }
    }
    None
}

fn find_bracket_object(
    text: &str,
    offset: usize,
    open: char,
    close: char,
    is_inner: bool,
) -> Option<TextObjectRange> {
    let mut depth = 0usize;
    let mut start: Option<usize> = None;
    let mut i = offset;
    loop {
        let c = match text.get(i..).and_then(|rest| rest.chars().next()) {
            Some(c) => c,
            None => break,
        };
        if i != offset && c == close {
            depth += 1;
        } else if c == open {
            if depth == 0 {
                start = Some(i);
                break;
            }
            depth = depth.saturating_sub(1);
        }
        if i == 0 {
            break;
        }
        i = text[..i]
            .char_indices()
            .next_back()
            .map(|(p, _)| p)
            .unwrap_or(0);
    }
    let start = start?;
    depth = 0;
    let mut end: Option<usize> = None;
    let mut j = start;
    while j < text.len() {
        let c = match text.get(j..).and_then(|rest| rest.chars().next()) {
            Some(c) => c,
            None => break,
        };
        if c == open && j != start {
            depth += 1;
        } else if c == close {
            if depth == 0 {
                end = Some(j);
                break;
            }
            depth = depth.saturating_sub(1);
        }
        j += c.len_utf8();
    }
    let end = end?;
    Some(if is_inner {
        TextObjectRange {
            start: start + open.len_utf8(),
            end,
        }
    } else {
        TextObjectRange {
            start,
            end: end + close.len_utf8(),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn inner(t: char) -> TextObjScope {
        let _ = t;
        TextObjScope::Inner
    }
    fn around(t: char) -> TextObjScope {
        let _ = t;
        TextObjScope::Around
    }
    #[test]
    fn word_objects_cover_inner_and_around() {
        let text = "hello world foo";
        // cursor on "world" (offset 6)
        let iw = find_text_object(text, 7, 'w', inner('i')).unwrap();
        assert_eq!(&text[iw.start..iw.end], "world");
        let aw = find_text_object(text, 7, 'w', around('a')).unwrap();
        assert_eq!(&text[aw.start..aw.end], "world ");
        // `aw` at line start attaches leading whitespace instead
        let text2 = "alpha beta";
        let aw2 = find_text_object(text2, 0, 'w', around('a')).unwrap();
        assert_eq!(&text2[aw2.start..aw2.end], "alpha ");
        // punctuation runs are their own object
        let text3 = "a, b";
        let obj = find_text_object(text3, 1, 'w', inner('i')).unwrap();
        assert_eq!(&text3[obj.start..obj.end], ",");
        // whitespace-only cursor selects the whitespace run
        let text4 = "a  b";
        let obj = find_text_object(text4, 1, 'w', inner('i')).unwrap();
        assert_eq!(&text4[obj.start..obj.end], "  ");
    }
    #[test]
    fn word_big_objects_treat_punct_as_word() {
        let text = "foo, bar";
        let obj = find_text_object(text, 3, 'W', inner('i')).unwrap();
        assert_eq!(&text[obj.start..obj.end], "foo,");
    }
    #[test]
    fn quote_objects_pair_within_line() {
        let text = "say \"hi there\" now";
        let obj = find_text_object(text, 7, '"', inner('i')).unwrap();
        assert_eq!(&text[obj.start..obj.end], "hi there");
        let obj = find_text_object(text, 7, '"', around('a')).unwrap();
        assert_eq!(&text[obj.start..obj.end], "\"hi there\"");
        // cursor outside any quote pair fails
        assert!(find_text_object(text, 1, '"', inner('i')).is_none());
    }
    #[test]
    fn bracket_objects_handle_nesting_and_alias_keys() {
        let text = "fn(a, (b, c), d)";
        let obj = find_text_object(text, 9, '(', inner('i')).unwrap();
        assert_eq!(&text[obj.start..obj.end], "b, c");
        let obj = find_text_object(text, 9, ')', around('a')).unwrap();
        assert_eq!(&text[obj.start..obj.end], "(b, c)");
        // outer pair from cursor on outer open
        let obj = find_text_object(text, 3, 'b', inner('i')).unwrap();
        assert_eq!(&text[obj.start..obj.end], "a, (b, c), d");
        let text2 = "x{a{b}c}y";
        let obj = find_text_object(text2, 4, 'B', inner('i')).unwrap();
        assert_eq!(&text2[obj.start..obj.end], "b");
        let text3 = "a<b>c";
        let obj = find_text_object(text3, 2, '<', around('a')).unwrap();
        assert_eq!(&text3[obj.start..obj.end], "<b>");
    }
}
