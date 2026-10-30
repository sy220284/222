//! Manual Rust port of all runtime exports in src/utils/stringUtils.ts.
//! String limits and offsets follow JavaScript UTF-16 code-unit semantics.
#![allow(dead_code, non_snake_case)]
use serde_json::{json, Value};

const MAX_STRING_LENGTH: usize = 1 << 25;
const TRUNCATION_MARKER: &str = "...[truncated]";

fn string_arg(args: &[Value], index: usize) -> &str {
    args.get(index).and_then(Value::as_str).expect("source export requires a string argument")
}
fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}
fn slice_utf16(s: &str, end: usize) -> String {
    String::from_utf16_lossy(&s.encode_utf16().take(end).collect::<Vec<_>>())
}

pub fn escapeRegExp(args: &[Value]) -> Value {
    let value = string_arg(args, 0);
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        if ".*+?^${}()|[]\\".contains(ch) {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    json!(escaped)
}

pub fn capitalize(args: &[Value]) -> Value {
    let value = string_arg(args, 0);
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return json!("");
    };
    // JS charAt(0) selects a UTF-16 code unit; normal BMP characters map
    // directly. A supplementary first character remains unchanged.
    if first.len_utf16() == 2 {
        return json!(value);
    }
    let mut result = first.to_uppercase().collect::<String>();
    result.push_str(chars.as_str());
    json!(result)
}

pub fn plural(args: &[Value]) -> Value {
    let word = string_arg(args, 1);
    if args.first().and_then(Value::as_f64) == Some(1.0) {
        json!(word)
    } else {
        json!(args.get(2).and_then(Value::as_str).map(str::to_owned)
            .unwrap_or_else(|| format!("{word}s")))
    }
}

pub fn firstLineOf(args: &[Value]) -> Value {
    json!(string_arg(args, 0).split_once('\n').map_or(string_arg(args, 0), |(before, _)| before))
}

pub fn countCharInString(args: &[Value]) -> Value {
    let s = string_arg(args, 0).encode_utf16().collect::<Vec<_>>();
    let needle = string_arg(args, 1).encode_utf16().collect::<Vec<_>>();
    let start = args.get(2).and_then(Value::as_i64).unwrap_or(0).max(0) as usize;
    if needle.is_empty() {
        return json!(if start <= s.len() { s.len() - start + 1 } else { 0 });
    }
    let count = (start..s.len())
        .filter(|&i| s.get(i..i.saturating_add(needle.len())) == Some(needle.as_slice()))
        .count();
    json!(count)
}

pub fn normalizeFullWidthDigits(args: &[Value]) -> Value {
    json!(string_arg(args, 0).chars().map(|ch| {
        if ('０'..='９').contains(&ch) {
            char::from_u32(ch as u32 - 0xfee0).expect("valid ASCII digit")
        } else { ch }
    }).collect::<String>())
}

pub fn normalizeFullWidthSpace(args: &[Value]) -> Value {
    json!(string_arg(args, 0).replace('\u{3000}', " "))
}

pub fn safeJoinLines(args: &[Value]) -> Value {
    let lines = args.first().and_then(Value::as_array).expect("source export requires string array");
    let delimiter = args.get(1).and_then(Value::as_str).unwrap_or(",");
    let max_size = args.get(2).and_then(Value::as_u64)
        .map_or(MAX_STRING_LENGTH, |n| usize::try_from(n).unwrap_or(usize::MAX));
    let mut result = String::new();
    let mut size = 0usize;
    for line in lines {
        let line = line.as_str().expect("all lines must be strings");
        // Source uses `result ? delimiter : ''`, which skips delimiters after
        // empty lines until actual content has been accumulated.
        let separator = if result.is_empty() { "" } else { delimiter };
        let extra = utf16_len(separator).saturating_add(utf16_len(line));
        if size.saturating_add(extra) <= max_size {
            result.push_str(separator);
            result.push_str(line);
            size += extra;
        } else {
            let remaining = max_size as i128 - size as i128
                - utf16_len(separator) as i128 - utf16_len(TRUNCATION_MARKER) as i128;
            if remaining > 0 {
                result.push_str(separator);
                result.push_str(&slice_utf16(line, remaining as usize));
            }
            result.push_str(TRUNCATION_MARKER);
            break;
        }
    }
    json!(result)
}

#[derive(Debug, Clone)]
pub struct EndTruncatingAccumulator {
    max_size: usize,
    content: String,
    truncated: bool,
    total_bytes: usize,
}

impl Default for EndTruncatingAccumulator {
    fn default() -> Self { Self::new(MAX_STRING_LENGTH) }
}
impl EndTruncatingAccumulator {
    pub fn new(max_size: usize) -> Self {
        Self { max_size, content: String::new(), truncated: false, total_bytes: 0 }
    }
    pub fn append(&mut self, data: &str) {
        let len = utf16_len(data);
        self.total_bytes = self.total_bytes.saturating_add(len);
        if self.truncated && self.length() >= self.max_size { return; }
        if self.length().saturating_add(len) > self.max_size {
            let remaining = self.max_size.saturating_sub(self.length());
            if remaining > 0 { self.content.push_str(&slice_utf16(data, remaining)); }
            self.truncated = true;
        } else {
            self.content.push_str(data);
        }
    }
    pub fn clear(&mut self) {
        self.content.clear();
        self.truncated = false;
        self.total_bytes = 0;
    }
    pub fn length(&self) -> usize { utf16_len(&self.content) }
    pub fn truncated(&self) -> bool { self.truncated }
    pub fn total_bytes(&self) -> usize { self.total_bytes }
    pub fn result(&self) -> String {
        if !self.truncated { return self.content.clone(); }
        let removed_kb = (self.total_bytes.saturating_sub(self.max_size) as f64 / 1024.0 + 0.5).floor() as usize;
        format!("{}\n... [output truncated - {removed_kb}KB removed]", self.content)
    }
}

pub fn truncateToLines(args: &[Value]) -> Value {
    let text = string_arg(args, 0);
    let limit = args.get(1).and_then(Value::as_i64).expect("source export requires maxLines");
    let lines = text.split('\n').collect::<Vec<_>>();
    if lines.len() as i64 <= limit { return json!(text); }
    let end = if limit >= 0 { (limit as usize).min(lines.len()) } else { lines.len().saturating_sub(limit.unsigned_abs() as usize) };
    json!(format!("{}…", lines[..end].join("\n")))
}

#[cfg(test)]
mod source_parity_tests {
    use super::*;
    #[test]
    fn string_exports_match_source_cases() {
        assert_eq!(escapeRegExp(&[json!("a.*[b]\\z")]), json!("a\\.\\*\\[b\\]\\\\z"));
        assert_eq!(capitalize(&[json!("fooBar")]), json!("FooBar"));
        assert_eq!(capitalize(&[json!("")]), json!(""));
        assert_eq!(plural(&[json!(1), json!("file")]), json!("file"));
        assert_eq!(plural(&[json!(3), json!("file")]), json!("files"));
        assert_eq!(plural(&[json!(2), json!("entry"), json!("entries")]), json!("entries"));
        assert_eq!(firstLineOf(&[json!("#!/bin/sh\necho hi")]), json!("#!/bin/sh"));
        assert_eq!(countCharInString(&[json!("ababa"), json!("aba")]), json!(2));
        assert_eq!(normalizeFullWidthDigits(&[json!("１２３abc")]), json!("123abc"));
        assert_eq!(normalizeFullWidthSpace(&[json!("a　b")]), json!("a b"));
        assert_eq!(safeJoinLines(&[json!(["foo","bar"]),json!(","),json!(8)]),json!("foo,bar"));
        assert_eq!(safeJoinLines(&[json!(["abcdef", "ghi"]),json!(","),json!(5)]),json!("...[truncated]"));
        assert_eq!(truncateToLines(&[json!("a\nb\nc"),json!(2)]), json!("a\nb…"));
        assert_eq!(truncateToLines(&[json!("a\nb"),json!(3)]), json!("a\nb"));
    }
    #[test]
    fn accumulator_preserves_original_head_and_total_on_overflow_and_clear() {
        let mut acc = EndTruncatingAccumulator::new(5);
        acc.append("abc"); acc.append("defgh"); acc.append("i");
        assert_eq!(acc.length(),5);
        assert_eq!(acc.total_bytes(),9);
        assert!(acc.truncated());
        assert_eq!(acc.result(),"abcde\n... [output truncated - 0KB removed]");
        acc.clear(); assert!(!acc.truncated()); assert_eq!(acc.total_bytes(),0);
        assert_eq!(acc.result(), "");
    }
    #[test]
    fn utf16_count_and_join_follow_original_js_length() {
        assert_eq!(countCharInString(&[json!("😀😀"),json!("😀")]),json!(2));
        let mut acc = EndTruncatingAccumulator::new(4);
        acc.append("😀a"); acc.append("bc");
        assert_eq!(acc.total_bytes(),5);
        assert_eq!(acc.length(),4);
        assert_eq!(acc.result(),"😀ab\n... [output truncated - 0KB removed]");
    }
}
