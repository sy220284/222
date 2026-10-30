//! Native Rust port derived from original: src/utils/collapseBackgroundBashNotifications.ts
//!
//! Collapses consecutive completed-background-bash task-notifications into
//! a single synthetic "N background commands completed" notification.
//! Failed/killed tasks and agent/workflow notifications are left alone;
//! monitor stream events have no `<status>` tag and never match.
//! Pass-through in verbose mode so ctrl+O shows each completion.
//!
//! `is_fullscreen_env_enabled` is injected by the caller (the fullscreen
//! port owns the tmux/iTerm2 probe logic); everything else is verbatim.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde_json::{json, Value};

pub const TASK_NOTIFICATION_TAG: &str = "task-notification";
pub const STATUS_TAG: &str = "status";
pub const SUMMARY_TAG: &str = "summary";
/// Prefix that distinguishes bash-kind LocalShellTask completions from
/// agent/workflow/monitor notifications (original:
/// `BACKGROUND_BASH_SUMMARY_PREFIX` in LocalShellTask.ts).
pub const BACKGROUND_BASH_SUMMARY_PREFIX: &str = "Background command";

/// Extracts the inner text of `<tag>...</tag>` from a flat XML-ish string
/// (mirror of `extractTag` in utils/messages.ts: non-greedy match).
pub fn extract_tag(text: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = text.find(&open)? + open.len();
    let end = start + text[start..].find(&close)?;
    Some(text[start..end].to_string())
}

/// Renderable message: the subset of fields this module touches.
/// `message.content[0].text` of a `user` message carries the XML tags.
fn user_first_text(msg: &Value) -> Option<&str> {
    let role = msg.get("message")?.get("role")?.as_str()?;
    if role != "user" {
        return None;
    }
    let content = msg.get("message")?.get("content")?.as_array()?;
    let first = content.first()?;
    let kind = first.get("type")?.as_str()?;
    if kind != "text" {
        return None;
    }
    first.get("text")?.as_str()
}

fn is_completed_background_bash(msg: &Value) -> bool {
    let Some(text) = user_first_text(msg) else {
        return false;
    };
    if !text.contains(&format!("<{TASK_NOTIFICATION_TAG}")) {
        return false;
    }
    // Only collapse successful completions — failed/killed stay visible.
    if extract_tag(text, STATUS_TAG).as_deref() != Some("completed") {
        return false;
    }
    // The prefix constant distinguishes bash-kind completions.
    extract_tag(text, SUMMARY_TAG)
        .map(|summary| summary.starts_with(BACKGROUND_BASH_SUMMARY_PREFIX))
        .unwrap_or(false)
}

/// Collapses consecutive completed-background-bash notifications.
/// `fullscreen_env_enabled` mirrors the original's `isFullscreenEnvEnabled()`
/// gate; pass-through when false or when `verbose` is set.
pub fn collapse_background_bash_notifications(
    messages: &[Value],
    verbose: bool,
    fullscreen_env_enabled: bool,
) -> Vec<Value> {
    if !fullscreen_env_enabled || verbose {
        return messages.to_vec();
    }

    let mut result: Vec<Value> = Vec::new();
    let mut i = 0usize;

    while i < messages.len() {
        let msg = &messages[i];
        if is_completed_background_bash(msg) {
            let mut count = 0usize;
            while i < messages.len() && is_completed_background_bash(&messages[i]) {
                count += 1;
                i += 1;
            }
            if count == 1 {
                result.push(msg.clone());
            } else {
                // Synthesize a task-notification the existing renderer knows.
                let text = format!(
                    "<{TASK_NOTIFICATION_TAG}><{STATUS_TAG}>completed</{STATUS_TAG}><{SUMMARY_TAG}>{count} background commands completed</{SUMMARY_TAG}></{TASK_NOTIFICATION_TAG}>"
                );
                let mut synthetic = msg.clone();
                synthetic["message"] = json!({
                    "role": "user",
                    "content": [{ "type": "text", "text": text }]
                });
                result.push(synthetic);
            }
        } else {
            result.push(msg.clone());
            i += 1;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn bash_completion(summary: &str) -> Value {
        json!({
            "type": "user",
            "message": {
                "role": "user",
                "content": [{
                    "type": "text",
                    "text": format!("<task-notification><status>completed</status><summary>{summary}</summary></task-notification>")
                }]
            }
        })
    }

    #[test]
    fn collapses_consecutive_completions() {
        let msgs = vec![
            bash_completion("Background command echo done"),
            bash_completion("Background command ls done"),
            json!({"type": "assistant"}),
        ];
        let out = collapse_background_bash_notifications(&msgs, false, true);
        assert_eq!(out.len(), 2);
        let text = out[0]["message"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("2 background commands completed"));
    }

    #[test]
    fn keeps_single_and_failed_notifications() {
        let msgs = vec![
            bash_completion("Background command only-one"),
            json!({
                "type": "user",
                "message": { "role": "user", "content": [{
                    "type": "text",
                    "text": "<task-notification><status>failed</status><summary>Background command x</summary></task-notification>"
                }]}
            }),
        ];
        let out = collapse_background_bash_notifications(&msgs, false, true);
        assert_eq!(out.len(), 2);
        assert!(out[0]["message"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Background command only-one"));
    }

    #[test]
    fn passthrough_when_verbose_or_not_fullscreen() {
        let msgs = vec![
            bash_completion("Background command a"),
            bash_completion("Background command b"),
        ];
        assert_eq!(
            collapse_background_bash_notifications(&msgs, true, true).len(),
            2
        );
        assert_eq!(
            collapse_background_bash_notifications(&msgs, false, false).len(),
            2
        );
    }

    #[test]
    fn extract_tag_non_greedy() {
        let text = "<status>completed</status><summary>x</summary>";
        assert_eq!(extract_tag(text, "status").as_deref(), Some("completed"));
        assert_eq!(extract_tag(text, "summary").as_deref(), Some("x"));
        assert_eq!(extract_tag(text, "missing"), None);
    }
}
