//! Native Rust port derived from original: src/utils/tokens.ts
//!
//! Token accounting from API usage payloads. Synthetic-message and
//! synthetic-model markers are injected as tag sets (their values live
//! with the messages port). The iterations fallback semantics (#304930:
//! final window = non-cache input + output, consistent across both
//! branches) are verbatim, including the doc-comment rationale.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde_json::Value;

/// Marker: synthetic model name (original SYNTHETIC_MODEL tag set).
pub const SYNTHETIC_MODEL_MARKER: &str = "<synthetic>";

/// Synthetic text markers (original SYNTHETIC_MESSAGES tag set; values
/// owned by the messages port — empty here means none are matched).
pub const SYNTHETIC_TEXT_MARKERS: &[&str] = &[];

/// `getTokenUsage`: assistant + has usage + not synthetic text/model.
pub fn get_token_usage(message: &Value) -> Option<&Value> {
    if message.get("type").and_then(|v| v.as_str()) != Some("assistant") {
        return None;
    }
    let inner = message.get("message")?;
    if inner.get("usage").is_none() {
        return None;
    }
    if inner.get("model").and_then(|v| v.as_str()) == Some(SYNTHETIC_MODEL_MARKER) {
        return None;
    }
    // Synthetic-message gate: only text content whose exact text matches
    // the messages-port tag set is synthetic; normal text passes.
    let synthetic = inner
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .filter(|b| b.get("type").and_then(|v| v.as_str()) == Some("text"))
        .and_then(|b| b.get("text"))
        .and_then(|t| t.as_str())
        .map(|t| SYNTHETIC_TEXT_MARKERS.contains(&t))
        .unwrap_or(false);
    if synthetic {
        return None;
    }
    inner.get("usage")
}

/// Full context window size: input + cache creation + cache read + output.
pub fn get_token_count_from_usage(usage: &Value) -> i64 {
    let g = |k: &str| usage.get(k).and_then(|v| v.as_i64()).unwrap_or(0);
    g("input_tokens")
        + g("cache_creation_input_tokens")
        + g("cache_read_input_tokens")
        + g("output_tokens")
}

fn usage_i64(usage: &Value, key: &str) -> i64 {
    usage.get(key).and_then(|v| v.as_i64()).unwrap_or(0)
}

/// Scan backwards for the last real usage and compute the context size.
pub fn token_count_from_last_api_response(messages: &[Value]) -> i64 {
    for message in messages.iter().rev() {
        if let Some(usage) = get_token_usage(message) {
            return get_token_count_from_usage(usage);
        }
    }
    0
}

/// Final window from usage.iterations[-1] (input + output, no cache),
/// falling back to top-level input + output when iterations is absent —
/// both paths match #304930's formula so the two branches stay consistent.
pub fn final_context_tokens_from_last_response(messages: &[Value]) -> i64 {
    for message in messages.iter().rev() {
        if let Some(usage) = get_token_usage(message) {
            let iterations = usage.get("iterations").and_then(|v| v.as_array());
            if let Some(last) = iterations.and_then(|it| it.last()) {
                return usage_i64(last, "input_tokens") + usage_i64(last, "output_tokens");
            }
            return usage_i64(usage, "input_tokens") + usage_i64(usage, "output_tokens");
        }
    }
    0
}

/// Output tokens only (never use for threshold comparisons upstream note).
pub fn output_tokens_from_last_response(messages: &[Value]) -> i64 {
    for message in messages.iter().rev() {
        if let Some(usage) = get_token_usage(message) {
            return usage_i64(usage, "output_tokens");
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn assistant(usage: Value) -> Value {
        json!({
            "type": "assistant",
            "message": {
                "id": "msg_1",
                "model": "test-model",
                "content": [{"type": "text", "text": "hi"}],
                "usage": usage
            }
        })
    }

    #[test]
    fn usage_count_includes_all_four_buckets() {
        let usage = json!({
            "input_tokens": 100,
            "cache_creation_input_tokens": 10,
            "cache_read_input_tokens": 5,
            "output_tokens": 20
        });
        assert_eq!(get_token_count_from_usage(&usage), 135);
        assert_eq!(
            get_token_count_from_usage(&json!({"input_tokens": 10, "output_tokens": 5})),
            15
        );
    }

    #[test]
    fn synthetic_messages_excluded() {
        let real = vec![assistant(json!({"input_tokens": 50, "output_tokens": 10}))];
        assert_eq!(token_count_from_last_api_response(&real), 60);
        let synthetic = vec![json!({
            "type": "assistant",
            "message": {
                "model": SYNTHETIC_MODEL_MARKER,
                "content": [{"type": "text", "text": "hi"}],
                "usage": {"input_tokens": 999, "output_tokens": 999}
            }
        })];
        assert_eq!(token_count_from_last_api_response(&synthetic), 0);
    }

    #[test]
    fn iterations_final_window_excludes_cache() {
        let msgs = vec![assistant(json!({
            "input_tokens": 100,
            "output_tokens": 20,
            "iterations": [
                {"input_tokens": 50, "output_tokens": 10},
                {"input_tokens": 80, "output_tokens": 15}
            ]
        }))];
        assert_eq!(final_context_tokens_from_last_response(&msgs), 95);
        let no_iter = vec![assistant(json!({
            "input_tokens": 100,
            "cache_read_input_tokens": 30,
            "output_tokens": 20
        }))];
        assert_eq!(final_context_tokens_from_last_response(&no_iter), 120);
    }

    #[test]
    fn output_only_scan() {
        let msgs = vec![
            json!({"type": "user", "message": {"content": []}}),
            assistant(json!({"input_tokens": 100, "output_tokens": 42})),
        ];
        assert_eq!(output_tokens_from_last_response(&msgs), 42);
    }
}

/// Registry-view wrapper preserving the original export name.
pub fn getTokenCountFromUsage(args: &[Value]) -> Value {
    match args.first() {
        Some(u) => serde_json::json!(get_token_count_from_usage(u)),
        None => serde_json::json!(0),
    }
}
