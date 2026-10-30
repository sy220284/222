//! Shared 2.1.88 text-input semantics. The functions in this module are
//! deliberately independent of a model vendor and are reused by the terminal
//! input views and the source-inspection entry point.
use serde_json::{json, Value};

fn bool_prop(props: &Value, key: &str) -> bool {
    props.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// Port of `src/hooks/renderPlaceholder.ts::renderPlaceholder`.
/// `invert` is the terminal inverse-video ANSI SGR style, as used by chalk.
/// The optional `invert` callback in the original is a host concern, and is
/// represented here by the default terminal inverse style.
pub fn render_placeholder(props: &Value) -> Value {
    let placeholder = props.get("placeholder").and_then(Value::as_str).unwrap_or("");
    let value = props.get("value").and_then(Value::as_str).unwrap_or("");
    let show_cursor = bool_prop(props, "showCursor") && bool_prop(props, "focus")
        && props.get("terminalFocus").and_then(Value::as_bool).unwrap_or(true);
    let rendered = if placeholder.is_empty() {
        Value::Null
    } else if bool_prop(props, "hidePlaceholderText") {
        json!(if show_cursor { "\u{1b}[7m \u{1b}[27m" } else { "" })
    } else if show_cursor {
        // JS placeholder[0]/slice(1) are UTF-16 based. For normal Unicode
        // scalars keep the first character together rather than emit an
        // invalid UTF-16 surrogate into an otherwise UTF-8 Rust string.
        let mut chars = placeholder.chars();
        let first = chars.next().unwrap_or(' ');
        let remainder = chars.as_str();
        json!(format!("\u{1b}[7m{first}\u{1b}[27m\u{1b}[2m{remainder}\u{1b}[22m"))
    } else {
        json!(format!("\u{1b}[2m{placeholder}\u{1b}[22m"))
    };
    json!({"renderedPlaceholder": rendered, "showPlaceholder": value.is_empty() && !placeholder.is_empty()})
}

fn highlight_start(h: &Value) -> usize {
    h.get("start").and_then(Value::as_u64).unwrap_or(0) as usize
}
fn highlight_end(h: &Value) -> usize {
    h.get("end").and_then(Value::as_u64).unwrap_or(0) as usize
}
fn highlight_priority(h: &Value) -> i64 {
    h.get("priority").and_then(Value::as_i64).unwrap_or(0)
}

/// Find the byte index of a UTF-16 position in a Rust UTF-8 string. If the
/// position falls inside a surrogate pair, the scalar stays intact.
fn utf16_byte_offset(s: &str, wanted: usize) -> usize {
    let mut units = 0;
    for (byte, ch) in s.char_indices() {
        if units >= wanted { return byte; }
        units += ch.len_utf16();
        if units > wanted { return byte + ch.len_utf8(); }
    }
    s.len()
}

/// Break visible text at positions, recognizing ANSI escape sequences as
/// zero-width. ANSI styles are repeated at segment boundaries so that a
/// colored span remains self-contained when used by the Ink renderer.
fn segment_visible_ansi(text: &str, start: usize, end: usize) -> String {
    let mut pos = 0usize;
    let mut i = 0usize;
    let mut output = String::new();
    let mut active_sgr: Vec<String> = Vec::new();
    let bytes = text.as_bytes();
    let mut emitted_any_visible = false;
    while i < bytes.len() {
        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            let mut j = i + 2;
            while j < bytes.len() && !(0x40..=0x7e).contains(&bytes[j]) { j += 1; }
            if j < bytes.len() {
                let code = &text[i..=j];
                if bytes[j] == b'm' {
                    let params = &text[i+2..j];
                    if params.is_empty() || params == "0" {
                        active_sgr.clear();
                    } else {
                        active_sgr.push(code.to_string());
                    }
                    if pos > start && pos < end { output.push_str(code); }
                }
                i = j + 1;
                continue;
            }
        }
        let ch = text[i..].chars().next().expect("UTF-8 boundary");
        let ch_units = ch.len_utf16();
        if pos < end && pos + ch_units > start {
            if !emitted_any_visible {
                // At the first visible cell, emit any active styles inherited
                // from the prefix that was outside the selected segment.
                if output.is_empty() {
                    for code in &active_sgr { output.push_str(code); }
                }
                emitted_any_visible = true;
            }
            output.push(ch);
        }
        pos += ch_units;
        i += ch.len_utf8();
        if pos >= end { break; }
    }
    if emitted_any_visible && !active_sgr.is_empty() { output.push_str("\u{1b}[0m"); }
    output
}

fn visible_len(text: &str) -> usize {
    let mut total = 0;
    let mut i = 0;
    let b = text.as_bytes();
    while i < b.len() {
        if b[i] == 0x1b && i+1 < b.len() && b[i+1] == b'[' {
            let mut j = i+2;
            while j < b.len() && !(0x40..=0x7e).contains(&b[j]) { j += 1; }
            if j < b.len() { i = j+1; continue; }
        }
        let ch = text[i..].chars().next().expect("UTF-8 boundary");
        total += ch.len_utf16();
        i += ch.len_utf8();
    }
    total
}

/// Port of `src/utils/textHighlighting.ts::segmentTextByHighlights`.
/// Position and priority resolution follow the original algorithm: sort by
/// start ascending, priority descending, then reject overlapping ranges.
pub fn segment_text_by_highlights(text: &str, highlights: &[Value]) -> Vec<Value> {
    if highlights.is_empty() { return vec![json!({"text":text,"start":0})]; }
    let mut sorted = highlights.to_vec();
    sorted.sort_by(|a,b| highlight_start(a).cmp(&highlight_start(b))
        .then_with(|| highlight_priority(b).cmp(&highlight_priority(a))));
    let mut used = Vec::<(usize,usize)>::new();
    let mut selected = Vec::<Value>::new();
    for h in sorted {
        let a = highlight_start(&h);
        let b = highlight_end(&h);
        if a == b { continue; }
        let overlaps = used.iter().any(|&(s,e)|
            (a >= s && a < e) || (b > s && b <= e) || (a <= s && b >= e));
        if !overlaps {
            selected.push(h);
            used.push((a,b));
        }
    }
    let len = visible_len(text);
    let mut out = Vec::new();
    let mut pos = 0;
    for h in selected {
        let start = highlight_start(&h).min(len);
        let end = highlight_end(&h).min(len);
        if start > pos {
            let before = segment_visible_ansi(text, pos, start);
            if !before.is_empty() { out.push(json!({"text":before,"start":pos})); }
            pos = start;
        }
        if end > pos {
            let highlighted = segment_visible_ansi(text, pos, end);
            if !highlighted.is_empty() {
                out.push(json!({"text":highlighted,"start":pos,"highlight":h}));
            }
            pos = end;
        }
    }
    if pos < len {
        let after = segment_visible_ansi(text,pos,len);
        if !after.is_empty() { out.push(json!({"text":after,"start":pos})); }
    }
    out
}

/// Mirrors the original input cursor filtering and viewport projection used
/// by `BaseTextInput` (both dimensions use the source's UTF-16 offsets).
pub fn viewport_highlights(props: &Value) -> Vec<Value> {
    let highlights = props.get("highlights").and_then(Value::as_array).cloned().unwrap_or_default();
    let cursor = props.get("cursorOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let filtered: Vec<_> = highlights.into_iter().filter(|h| {
        !bool_prop(props,"showCursor") || h.get("dimColor").and_then(Value::as_bool).unwrap_or(false)
            || cursor < highlight_start(h) || cursor >= highlight_end(h)
    }).collect();
    let state = props.get("inputState").unwrap_or(&Value::Null);
    let start = state.get("viewportCharOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let end = state.get("viewportCharEnd").and_then(Value::as_u64).unwrap_or(usize::MAX as u64) as usize;
    if start == 0 { return filtered; }
    filtered.into_iter().filter_map(|mut h| {
        if highlight_end(&h) <= start || highlight_start(&h) >= end { return None; }
        let from = highlight_start(&h).saturating_sub(start);
        let to = highlight_end(&h).saturating_sub(start);
        h["start"] = json!(from);
        h["end"] = json!(to);
        Some(h)
    }).collect()
}

#[cfg(test)]
mod parity_tests {
    use super::*;
    #[test]
    fn placeholder_focus_and_hidden_recording_cases() {
        let p = render_placeholder(&json!({"placeholder":"hello","value":"","showCursor":true,"focus":true,"terminalFocus":true}));
        assert_eq!(p["showPlaceholder"], true);
        assert!(p["renderedPlaceholder"].as_str().unwrap().contains("\u{1b}[7mh"));
        assert_eq!(render_placeholder(&json!({"placeholder":"abc","value":"text"}))["showPlaceholder"], false);
        assert_eq!(render_placeholder(&json!({"placeholder":"abc","value":"","hidePlaceholderText":true}))["renderedPlaceholder"], "");
        assert_eq!(render_placeholder(&json!({"placeholder":"","value":""}))["renderedPlaceholder"], Value::Null);
    }
    #[test]
    fn highlights_exclude_overlaps_in_source_priority_order() {
        let highlights = [json!({"start":0,"end":2,"priority":1}),
           json!({"start":0,"end":3,"priority":2}), json!({"start":3,"end":4,"priority":0})];
        let result=segment_text_by_highlights("abcd!", &highlights);
        assert_eq!(result.len(),3);
        assert_eq!(result[0]["text"],"abc");
        assert_eq!(result[0]["highlight"]["priority"], 2);
        assert_eq!(result[1]["text"],"d");
        assert_eq!(result[2]["text"],"!");
        assert_eq!(segment_text_by_highlights("abc",&[]), vec![json!({"text":"abc","start":0})]);
    }
    #[test]
    fn highlights_use_visible_offsets_and_remain_ansi_styled() {
        let r=segment_text_by_highlights("\u{1b}[31mabcd\u{1b}[0m", &[json!({"start":1,"end":3,"priority":1})]);
        assert_eq!(r.len(), 3);
        assert!(r[1]["text"].as_str().unwrap().contains("bc"));
        assert!(r[1]["text"].as_str().unwrap().contains("\u{1b}[31m"));
        assert_eq!(r[1]["start"],1);
    }
    #[test]
    fn highlight_viewport_and_cursor_semantics() {
        let p=json!({"showCursor":true,"cursorOffset":2,"inputState":{"viewportCharOffset":2,"viewportCharEnd":8},
         "highlights":[{"start":1,"end":4,"priority":1},{"start":4,"end":7,"priority":1},{"start":0,"end":10,"dimColor":true,"priority":2}]});
        let a=viewport_highlights(&p);
        assert_eq!(a.len(),2);
        assert_eq!(a[0]["start"],2);
        assert_eq!(a[1]["start"],0);
    }
    #[test]
    fn utf16_positions_match_original_string_length() {
        assert_eq!(utf16_byte_offset("😀abc",2),4);
        let r=segment_text_by_highlights("😀ab",&[json!({"start":2,"end":3,"priority":1})]);
        assert_eq!(r[1]["text"],"a");
    }
}
