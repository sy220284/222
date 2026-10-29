//! Original message coalescing and terminal text semantics from 2.1.88:
//! src/utils/collapseHookSummaries.ts, collapseTeammateShutdowns.ts,
//! src/ink/tabstops.ts, src/ink/measure-text.ts, src/ink/termio/esc.ts.
use crate::portable_ink_core::LineWidthCache;
use serde_json::{json, Value};

fn is_shutdown(message: &Value) -> bool {
    message["type"] == "attachment"
        && message["attachment"]["type"] == "task_status"
        && message["attachment"]["taskType"] == "in_process_teammate"
        && message["attachment"]["status"] == "completed"
}

pub fn collapse_teammate_shutdowns(messages: &[Value]) -> Vec<Value> {
    let mut result = Vec::with_capacity(messages.len());
    let mut index = 0;
    while index < messages.len() {
        let first = &messages[index];
        if is_shutdown(first) {
            let start = index;
            while index < messages.len() && is_shutdown(&messages[index]) {
                index += 1;
            }
            let count = index - start;
            if count == 1 {
                result.push(first.clone());
            } else {
                result.push(json!({
                    "type":"attachment",
                    "uuid":first["uuid"],
                    "timestamp":first["timestamp"],
                    "attachment":{"type":"teammate_shutdown_batch","count":count}
                }));
            }
        } else {
            result.push(first.clone());
            index += 1;
        }
    }
    result
}

fn labeled_hook(message: &Value) -> Option<&str> {
    if message["type"] != "system" || message["subtype"] != "stop_hook_summary" {
        return None;
    }
    message["hookLabel"].as_str()
}

pub fn collapse_hook_summaries(messages: &[Value]) -> Vec<Value> {
    let mut result = Vec::with_capacity(messages.len());
    let mut index = 0;
    while index < messages.len() {
        let first = &messages[index];
        if let Some(label) = labeled_hook(first) {
            let start = index;
            while index < messages.len() && labeled_hook(&messages[index]) == Some(label) {
                index += 1;
            }
            if index - start == 1 {
                result.push(first.clone());
                continue;
            }
            let mut merged = first.clone();
            let mut count = 0u64;
            let mut infos = Vec::new();
            let mut errors = Vec::new();
            let mut prevented = false;
            let mut output = false;
            let mut duration = json!(0);
            for message in &messages[start..index] {
                count += message["hookCount"].as_u64().unwrap_or(0);
                if let Some(values) = message["hookInfos"].as_array() {
                    infos.extend(values.iter().cloned());
                }
                if let Some(values) = message["hookErrors"].as_array() {
                    errors.extend(values.iter().cloned());
                }
                prevented |= message["preventedContinuation"].as_bool().unwrap_or(false);
                output |= message["hasOutput"].as_bool().unwrap_or(false);
                if message["totalDurationMs"].as_f64().unwrap_or(0.0)
                    > duration.as_f64().unwrap_or(0.0)
                {
                    duration = message["totalDurationMs"].clone();
                }
            }
            merged["hookCount"] = json!(count);
            merged["hookInfos"] = Value::Array(infos);
            merged["hookErrors"] = Value::Array(errors);
            merged["preventedContinuation"] = json!(prevented);
            merged["hasOutput"] = json!(output);
            merged["totalDurationMs"] = duration;
            result.push(merged);
        } else {
            result.push(first.clone());
            index += 1;
        }
    }
    result
}

pub fn measure_text(text: &str, max_width: f64, cache: &mut LineWidthCache) -> Value {
    if text.is_empty() {
        return json!({"width":0,"height":0});
    }
    let no_wrap = max_width <= 0.0 || !max_width.is_finite();
    let mut width = 0usize;
    let mut height = 0usize;
    for line in text.split('\n') {
        let line_width = cache.line_width(line);
        width = width.max(line_width);
        height += if no_wrap || line_width == 0 {
            1
        } else {
            ((line_width as f64 / max_width).ceil() as usize).max(1)
        };
    }
    json!({"width":width,"height":height})
}

pub fn parse_esc(chars: &str) -> Option<Value> {
    let first = chars.chars().next()?;
    match first {
        'c' => Some(json!({"type":"reset"})),
        '7' => Some(json!({"type":"cursor","action":{"type":"save"}})),
        '8' => Some(json!({"type":"cursor","action":{"type":"restore"}})),
        'D' => Some(json!({"type":"cursor","action":{"type":"move","direction":"down","count":1}})),
        'M' => Some(json!({"type":"cursor","action":{"type":"move","direction":"up","count":1}})),
        'E' => Some(json!({"type":"cursor","action":{"type":"nextLine","count":1}})),
        'H' => None,
        '(' | ')' if chars.chars().count() >= 2 => None,
        _ => Some(json!({"type":"unknown","sequence":format!("\u{1b}{chars}")})),
    }
}

/// Consume a CSI, OSC or DCS sequence without changing screen columns.
/// The original terminal tokenizer exposes sequences as separate tokens,
/// preserving bytes while text/tab expansion advances only visible width.
fn ansi_sequence_end(text: &str, start: usize) -> Option<usize> {
    let b = text.as_bytes();
    if b.get(start) != Some(&0x1b) {
        return None;
    }
    let next = *b.get(start + 1)?;
    match next {
        b'[' => {
            let mut end = start + 2;
            while end < b.len() {
                if (0x40..=0x7e).contains(&b[end]) {
                    return Some(end + 1);
                }
                end += 1;
            }
            Some(b.len())
        }
        b']' | b'P' | b'^' | b'_' => {
            let mut end = start + 2;
            while end < b.len() {
                if b[end] == 0x07 || end + 1 < b.len() && b[end] == 0x1b && b[end + 1] == b'\\' {
                    return Some(if b[end] == 0x07 { end + 1 } else { end + 2 });
                }
                end += 1;
            }
            Some(b.len())
        }
        _ => Some((start + 2).min(b.len())),
    }
}

pub fn expand_tabs(text: &str, interval: usize) -> String {
    if !text.contains('\t') || interval == 0 {
        return text.to_owned();
    }
    let mut output = String::with_capacity(text.len());
    let mut plain = String::new();
    let mut col = 0usize;
    let mut width = LineWidthCache::default();
    let mut offset = 0;
    while offset < text.len() {
        if let Some(end) = ansi_sequence_end(text, offset) {
            if !plain.is_empty() {
                col += width.line_width(&plain);
                output.push_str(&plain);
                plain.clear();
            }
            output.push_str(&text[offset..end]);
            offset = end;
            continue;
        }
        let ch = text[offset..].chars().next().unwrap();
        if ch == '\t' || ch == '\n' {
            if !plain.is_empty() {
                col += width.line_width(&plain);
                output.push_str(&plain);
                plain.clear();
            }
            if ch == '\n' {
                output.push('\n');
                col = 0;
            } else {
                let spaces = interval - col % interval;
                output.push_str(&" ".repeat(spaces));
                col += spaces;
            }
        } else {
            plain.push(ch);
        }
        offset += ch.len_utf8();
    }
    output.push_str(&plain);
    output
}

pub fn inspect(source: &str, export: &str, args: &[Value]) -> Option<Value> {
    let first = args.first().unwrap_or(&Value::Null);
    match (source, export) {
        ("src/utils/collapseTeammateShutdowns.ts", "collapseTeammateShutdowns") => {
            Some(json!(collapse_teammate_shutdowns(first.as_array()?)))
        }
        ("src/utils/collapseHookSummaries.ts", "collapseHookSummaries") => {
            Some(json!(collapse_hook_summaries(first.as_array()?)))
        }
        ("src/ink/measure-text.ts", "default") => {
            let mut cache = LineWidthCache::default();
            Some(measure_text(
                first.as_str()?,
                args.get(1)?.as_f64()?,
                &mut cache,
            ))
        }
        ("src/ink/tabstops.ts", "expandTabs") => Some(json!(expand_tabs(
            first.as_str()?,
            args.get(1).and_then(Value::as_u64).unwrap_or(8) as usize
        ))),
        ("src/ink/termio/esc.ts", "parseEsc") => {
            Some(parse_esc(first.as_str()?).unwrap_or(Value::Null))
        }
        _ => None,
    }
}

pub fn exports(source: &str) -> Option<&'static [&'static str]> {
    match source {
        "src/utils/collapseTeammateShutdowns.ts" => Some(&["collapseTeammateShutdowns"]),
        "src/utils/collapseHookSummaries.ts" => Some(&["collapseHookSummaries"]),
        "src/ink/measure-text.ts" => Some(&["default"]),
        "src/ink/tabstops.ts" => Some(&["expandTabs"]),
        "src/ink/termio/esc.ts" => Some(&["parseEsc"]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shutdown_runs_only_merge_consecutive_completed_teammates() {
        let one = json!({"type":"attachment","uuid":"first","timestamp":2,
            "attachment":{"type":"task_status","taskType":"in_process_teammate","status":"completed"}});
        let separate = json!({"type":"system","subtype":"info"});
        assert_eq!(
            collapse_teammate_shutdowns(&[one.clone()]),
            vec![one.clone()]
        );
        let result =
            collapse_teammate_shutdowns(&[one.clone(), one.clone(), separate.clone(), one]);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0]["attachment"]["type"], "teammate_shutdown_batch");
        assert_eq!(result[0]["attachment"]["count"], 2);
        assert_eq!(result[0]["uuid"], "first");
        assert_eq!(result[1], separate);
    }

    #[test]
    fn merge_hook_counts_and_parallel_wallclock_time() {
        let a = json!({"type":"system","subtype":"stop_hook_summary","hookLabel":"PostToolUse",
            "hookCount":2,"hookInfos":[1],"hookErrors":[],"totalDurationMs":100,
            "preventedContinuation":false,"hasOutput":true});
        let b = json!({"type":"system","subtype":"stop_hook_summary","hookLabel":"PostToolUse",
            "hookCount":3,"hookInfos":[2],"hookErrors":["bad"],"totalDurationMs":60,
            "preventedContinuation":true,"hasOutput":false});
        let merged = collapse_hook_summaries(&[a, b]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0]["hookCount"], 5);
        assert_eq!(merged[0]["hookInfos"], json!([1, 2]));
        assert_eq!(merged[0]["hookErrors"], json!(["bad"]));
        assert_eq!(merged[0]["totalDurationMs"], 100);
        assert_eq!(merged[0]["preventedContinuation"], true);
    }

    #[test]
    fn text_measure_tab_expansion_and_ansi_control_match_terminal_rules() {
        let mut cache = LineWidthCache::default();
        assert_eq!(
            measure_text("ABCD\n", 2.0, &mut cache),
            json!({"width":4,"height":3})
        );
        assert_eq!(
            measure_text("ABCD\n", f64::INFINITY, &mut cache),
            json!({"width":4,"height":2})
        );
        assert_eq!(expand_tabs("a\tb\n\tc", 8), "a       b\n        c");
        assert_eq!(
            expand_tabs("\u{1b}[31ma\tb\u{1b}[0m", 8),
            "\u{1b}[31ma       b\u{1b}[0m"
        );
        assert_eq!(
            parse_esc("7"),
            Some(json!({"type":"cursor","action":{"type":"save"}}))
        );
        assert_eq!(parse_esc("H"), None);
    }
}
