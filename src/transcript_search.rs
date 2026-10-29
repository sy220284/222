//! Transcript text extraction and case-insensitive search.
//! Derived from 2.1.88 src/utils/transcriptSearch.ts.
//! Model-facing reminders, hidden thinking, and serialized tool metadata must
//! not create phantom matches in a transcript displayed to the user.
use crate::protocol::Message;
use serde_json::Value;

const INTERRUPT: &str = "[Request interrupted by user]";
const INTERRUPT_TOOL: &str = "[Request interrupted by user for tool use]";
const REMINDER_OPEN: &str = "<system-reminder>";
const REMINDER_CLOSE: &str = "</system-reminder>";

fn visible_string(value: &Value) -> Option<&str> {
    value.as_str()
}
fn text_blocks(value: &Value) -> String {
    value
        .as_array()
        .map(|blocks| {
            blocks
                .iter()
                .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|block| block.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}
fn visible_user_text(content: &Value, native_tool_result: Option<&Value>) -> String {
    if let Some(text) = visible_string(content) {
        return if matches!(text, INTERRUPT | INTERRUPT_TOOL) {
            String::new()
        } else {
            text.to_owned()
        };
    }
    let mut parts = Vec::new();
    if let Some(blocks) = content.as_array() {
        for block in blocks {
            match block.get("type").and_then(Value::as_str) {
                Some("text") => {
                    if let Some(text) = block.get("text").and_then(Value::as_str) {
                        if !matches!(text, INTERRUPT | INTERRUPT_TOOL) {
                            parts.push(text.to_owned());
                        }
                    }
                }
                Some("tool_result") => {
                    // The block.content is a model-facing serialization and
                    // may contain non-visible security/system reminders.
                    parts.push(native_tool_result.map(tool_result_search_text).unwrap_or_default());
                }
                _ => {}
            }
        }
    }
    parts.join("\n")
}
fn memories_text(memories: Option<&Value>) -> String {
    memories
        .and_then(Value::as_array)
        .map(|memories| {
            memories
                .iter()
                .filter_map(|item| item.get("content").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}
fn strip_system_reminders(mut text: String) -> String {
    while let Some(start) = text.find(REMINDER_OPEN) {
        let content_start = start + REMINDER_OPEN.len();
        let Some(relative_end) = text[content_start..].find(REMINDER_CLOSE) else {
            break;
        };
        let end = content_start + relative_end + REMINDER_CLOSE.len();
        text.replace_range(start..end, "");
    }
    text
}
pub fn tool_use_search_text(input: &Value) -> String {
    if !input.is_object() && !input.is_array() {
        return String::new();
    }
    let mut parts = Vec::new();
    for key in [
        "command", "pattern", "file_path", "path", "prompt",
        "description", "query", "url", "skill",
    ] {
        if let Some(text) = input.get(key).and_then(Value::as_str) {
            parts.push(text.to_owned());
        }
    }
    for key in ["args", "files"] {
        if let Some(items) = input.get(key).and_then(Value::as_array) {
            if items.iter().all(Value::is_string) {
                parts.push(items.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" "));
            }
        }
    }
    parts.join("\n")
}
pub fn tool_result_search_text(result: &Value) -> String {
    if let Some(s) = result.as_str() {
        return s.to_owned();
    }
    if !result.is_object() && !result.is_array() {
        return String::new();
    }
    if let Some(stdout) = result.get("stdout").and_then(Value::as_str) {
        let stderr = result.get("stderr").and_then(Value::as_str).unwrap_or("");
        return if stderr.is_empty() {
            stdout.to_owned()
        } else {
            format!("{stdout}\n{stderr}")
        };
    }
    if let Some(content) = result.pointer("/file/content").and_then(Value::as_str) {
        return content.to_owned();
    }
    let mut parts = Vec::new();
    for key in ["content", "output", "result", "text", "message"] {
        if let Some(text) = result.get(key).and_then(Value::as_str) {
            parts.push(text.to_owned());
        }
    }
    for key in ["filenames", "lines", "results"] {
        if let Some(items) = result.get(key).and_then(Value::as_array) {
            if items.iter().all(Value::is_string) {
                parts.push(items.iter().filter_map(Value::as_str).collect::<Vec<_>>().join("\n"));
            }
        }
    }
    parts.join("\n")
}
/// Extract the string actually visible in the original transcript renderer.
/// A RenderableMessage is represented by a JSON object to preserve all
/// source-specific user/assistant/attachment variants.
pub fn renderable_search_text(message: &Value) -> String {
    let raw = match message.get("type").and_then(Value::as_str) {
        Some("user") => {
            let content = message.pointer("/message/content").unwrap_or(&Value::Null);
            visible_user_text(content, message.get("toolUseResult"))
        }
        Some("assistant") => {
            let blocks = message.pointer("/message/content").and_then(Value::as_array);
            let mut parts = Vec::new();
            if let Some(blocks) = blocks {
                for block in blocks {
                    match block.get("type").and_then(Value::as_str) {
                        Some("text") => {
                            if let Some(text) = block.get("text").and_then(Value::as_str) {
                                parts.push(text.to_owned());
                            }
                        }
                        Some("tool_use") => parts.push(tool_use_search_text(
                            block.get("input").unwrap_or(&Value::Null),
                        )),
                        _ => {} // Hidden thinking and other non-rendered blocks
                    }
                }
            }
            parts.join("\n")
        }
        Some("attachment") => match message.pointer("/attachment/type").and_then(Value::as_str) {
            Some("relevant_memories") => memories_text(message.pointer("/attachment/memories")),
            Some("queued_command")
                if message.pointer("/attachment/commandMode").and_then(Value::as_str)
                    != Some("task-notification")
                    && message.pointer("/attachment/isMeta").and_then(Value::as_bool) != Some(true) =>
            {
                let prompt = message.pointer("/attachment/prompt").unwrap_or(&Value::Null);
                prompt.as_str().map(str::to_owned).unwrap_or_else(|| text_blocks(prompt))
            }
            _ => String::new(),
        },
        Some("collapsed_read_search") => memories_text(message.get("relevantMemories")),
        _ => String::new(),
    };
    strip_system_reminders(raw).to_lowercase()
}
/// Adapter for messages persisted by the native Rust Agent. The persisted
/// form already stores only displayable plain text and tool output, not the
/// original model-facing RenderableMessage wrapper.
pub fn stored_message_search_text(message: &Message) -> String {
    let mut parts = Vec::new();
    match message.role.as_str() {
        "user" | "assistant" => {
            if let Some(content) = &message.content {
                if !matches!(content.as_str(), INTERRUPT | INTERRUPT_TOOL) {
                    parts.push(content.to_owned());
                }
            }
            if let Some(blocks) = &message.content_blocks {
                for block in blocks {
                    match block.get("type").and_then(Value::as_str) {
                        Some("text") => {
                            if let Some(text) = block.get("text").and_then(Value::as_str) {
                                if !matches!(text, INTERRUPT | INTERRUPT_TOOL) {
                                    parts.push(text.to_owned());
                                }
                            }
                        }
                        Some("tool_use") => parts.push(tool_use_search_text(
                            block.get("input").unwrap_or(&Value::Null),
                        )),
                        _ => {}
                    }
                }
            }
            if let Some(calls) = &message.tool_calls {
                for call in calls {
                    if let Ok(input) = serde_json::from_str::<Value>(&call.function.arguments) {
                        parts.push(tool_use_search_text(&input));
                    }
                }
            }
        }
        "tool" => {
            if let Some(content) = &message.content {
                if let Ok(native) = serde_json::from_str::<Value>(content) {
                    let text = tool_result_search_text(&native);
                    if !text.is_empty() {
                        parts.push(text);
                    }
                } else {
                    parts.push(content.to_owned());
                }
            }
        }
        _ => {}
    }
    strip_system_reminders(parts.join("\n")).to_lowercase()
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    pub message_index: usize,
    pub role: String,
    pub excerpt: String,
}
/// Precompute normalized text once so repeated queries don't re-lowercase
/// large transcripts on each keypress (the original WeakMap optimization).
pub struct TranscriptIndex {
    messages: Vec<(usize, String, String)>,
}
impl TranscriptIndex {
    pub fn from_stored(messages: &[Message]) -> Self {
        Self {
            messages: messages
                .iter()
                .enumerate()
                .map(|(index, message)| {
                    (index + 1, message.role.clone(), stored_message_search_text(message))
                })
                .collect(),
        }
    }
    pub fn find(&self, query: &str) -> Vec<SearchHit> {
        let query = query.to_lowercase();
        if query.is_empty() {
            return Vec::new();
        }
        self.messages
            .iter()
            .filter(|(_, _, text)| text.contains(&query))
            .map(|(index, role, text)| {
                let position = text.find(&query).unwrap_or(0);
                let char_position = text[..position].chars().count();
                let start = char_position.saturating_sub(45);
                let excerpt: String = text.chars().skip(start).take(140).collect();
                SearchHit {
                    message_index: *index,
                    role: role.clone(),
                    excerpt: excerpt.replace('\n', " "),
                }
            })
            .collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn user_sentinals_and_embedded_reminders_never_create_phantom_matches() {
        let value = json!({"type":"user","message":{"content":[
            {"type":"text","text":"visible <system-reminder>Secret MALWARE</system-reminder> name"},
            {"type":"text","text":"[Request interrupted by user]"},
            {"type":"tool_result","content":"backgroundInfo malware"}
        ]},"toolUseResult":{"stdout":"command output","stderr":"warning"}});
        let result = renderable_search_text(&value);
        assert!(result.contains("visible  name"));
        assert!(result.contains("command output\nwarning"));
        assert!(!result.contains("malware"));
        assert!(!result.contains("interrupted"));
        assert!(!result.contains("backgroundinfo"));
    }
    #[test]
    fn assistant_tool_calls_are_visible_but_thinking_is_not() {
        let value = json!({"type":"assistant","message":{"content":[
            {"type":"thinking","thinking":"secret reasoning"},
            {"type":"text","text":"Check this"},
            {"type":"tool_use","input":{"command":"git status","args":["-s","--ignored"],"backgroundTaskId":"hidden"}}
        ]}});
        let result = renderable_search_text(&value);
        assert!(result.contains("git status"));
        assert!(result.contains("-s --ignored"));
        assert!(!result.contains("secret"));
        assert!(!result.contains("hidden"));
    }
    #[test]
    fn queued_prompts_and_memories_follow_original_visibility_guards() {
        let queued = json!({"type":"attachment","attachment":{"type":"queued_command","prompt":"next task"}});
        assert_eq!(renderable_search_text(&queued), "next task");
        let hidden = json!({"type":"attachment","attachment":{"type":"queued_command","isMeta":true,"prompt":"secret"}});
        assert_eq!(renderable_search_text(&hidden), "");
        let task_notification = json!({"type":"attachment","attachment":{"type":"queued_command","commandMode":"task-notification","prompt":"secret"}});
        assert_eq!(renderable_search_text(&task_notification), "");
        let memories = json!({"type":"collapsed_read_search","relevantMemories":[{"content":"Important path"}]});
        assert_eq!(renderable_search_text(&memories), "important path");
    }
    #[test]
    fn native_tool_results_do_not_index_unrendered_metadata() {
        let native = json!({"output":"visible result","rawOutputPath":"hidden secret","backgroundTaskId":"id"});
        assert_eq!(tool_result_search_text(&native), "visible result");
        let read = json!({"file":{"content":"file body"},"message":"other"});
        assert_eq!(tool_result_search_text(&read), "file body");
        assert_eq!(tool_result_search_text(&json!(false)), "");
    }
    #[test]
    fn session_search_indexes_visible_text_without_model_or_mutation() {
        let messages = vec![
            Message::text("system","hidden system instruction"),
            Message::text("user","Repair café <system-reminder>SECRET</system-reminder> header"),
            Message::text("assistant","I fixed the café"),
        ];
        let idx = TranscriptIndex::from_stored(&messages);
        let hits = idx.find("CAFÉ");
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].message_index, 2);
        assert_eq!(hits[1].message_index, 3);
        assert!(idx.find("secret").is_empty());
        assert!(idx.find("hidden system").is_empty());
        assert!(idx.find("").is_empty());
    }
}