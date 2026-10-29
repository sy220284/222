//! Files referenced by successful file read/edit/write tool calls in the active
//! conversation. Reconstructed from the persisted transcript so /files also
//! works after a session resume. This is not the original live file-state cache.
use crate::protocol::Message;
use std::{
    collections::HashMap,
    path::{Component, Path, PathBuf},
};

const MAX_TRACKED_FILES: usize = 100;

fn tracked_target(name: &str, args: &str) -> Option<String> {
    let path_field = match name {
        "read_file" | "FileRead" | "Read" | "write_file" | "FileWrite" | "Write" | "edit_file"
        | "FileEdit" | "Edit" => "path",
        "notebook_edit" | "NotebookEdit" => "notebook_path",
        _ => return None,
    };
    let input: serde_json::Value = serde_json::from_str(args).ok()?;
    input
        .get(path_field)
        .or_else(|| input.get("file_path"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn add_successful_file(recent: &mut Vec<(String, String)>, root: &Path, path: String) {
    let absolute = if Path::new(&path).is_absolute() {
        PathBuf::from(path)
    } else {
        root.join(path)
    };
    // Preserve the original file identity even if the user removed it
    // after a successful read, or the session is being resumed later.
    let canonical = absolute
        .canonicalize()
        .unwrap_or_else(|_| normalize_path(&absolute));
    let display = canonical
        .strip_prefix(root)
        .unwrap_or(&canonical)
        .to_string_lossy()
        .replace('\\', "/");
    let identity = canonical.to_string_lossy().to_string();
    recent.retain(|(key, _)| key != &identity);
    recent.insert(0, (identity, display));
    recent.truncate(MAX_TRACKED_FILES);
}

pub fn paths(messages: &[Message], root: &Path) -> Vec<String> {
    let mut pending: HashMap<String, String> = HashMap::new();
    // FileStateCache in 2.1.88 tracks successful reads and edits, most
    // recently accessed first. Reconstruct it from the saved transcript.
    let mut recent: Vec<(String, String)> = Vec::new();
    let canonical_root = root.canonicalize().unwrap_or_else(|_| normalize_path(root));

    for message in messages {
        if message.role == "assistant" {
            if let Some(calls) = message.tool_calls.as_ref() {
                for call in calls {
                    if let Some(path) =
                        tracked_target(&call.function.name, &call.function.arguments)
                    {
                        pending.insert(call.id.clone(), path);
                    }
                }
            }
            // SDK structured transcripts encode tool_use inside content
            // blocks instead of the OpenAI tool_calls field.
            if let Some(blocks) = message.content_blocks.as_ref() {
                for block in blocks {
                    if block.get("type").and_then(serde_json::Value::as_str) != Some("tool_use") {
                        continue;
                    }
                    let (Some(id), Some(name), Some(input)) = (
                        block.get("id").and_then(serde_json::Value::as_str),
                        block.get("name").and_then(serde_json::Value::as_str),
                        block.get("input"),
                    ) else {
                        continue;
                    };
                    if let Some(path) = tracked_target(name, &input.to_string()) {
                        pending.insert(id.to_owned(), path);
                    }
                }
            }
        }

        if message.role == "tool" {
            if let Some(id) = message.tool_call_id.as_deref() {
                // Prefer the structured result below when the provider
                // includes content blocks alongside a tool-call identifier.
                let has_structured_result = message.content_blocks.as_ref().is_some_and(|blocks| {
                    blocks.iter().any(|block| {
                        block.get("type").and_then(serde_json::Value::as_str) == Some("tool_result")
                            && block.get("tool_use_id").and_then(serde_json::Value::as_str)
                                == Some(id)
                    })
                });
                if !has_structured_result {
                    if let Some(path) = pending.remove(id) {
                        let failed = message.content.as_deref().is_none_or(|content| {
                            let trimmed = content.trim_start();
                            trimmed.starts_with("ERROR:") || trimmed.starts_with("Error:")
                        });
                        if !failed {
                            add_successful_file(&mut recent, &canonical_root, path);
                        }
                    }
                }
            }
        }
        // In the original SDK wire format, tool_result is a user content
        // block; a successful result can be text, image, or structured data.
        if let Some(blocks) = message.content_blocks.as_ref() {
            for block in blocks {
                if block.get("type").and_then(serde_json::Value::as_str) != Some("tool_result") {
                    continue;
                }
                let Some(id) = block.get("tool_use_id").and_then(serde_json::Value::as_str) else {
                    continue;
                };
                if let Some(path) = pending.remove(id) {
                    if block.get("is_error").and_then(serde_json::Value::as_bool) != Some(true) {
                        add_successful_file(&mut recent, &canonical_root, path);
                    }
                }
            }
        }
    }
    recent.into_iter().map(|(_, display)| display).collect()
}
pub fn render(messages: &[Message], root: &Path) -> String {
    let files = paths(messages, root);
    if files.is_empty() {
        "No files in context".to_owned()
    } else {
        format!("Files in context:\n{}", files.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{FunctionCall, ToolCall};

    fn tool(id: &str, name: &str, path: &str) -> Message {
        let arguments = if name == "NotebookEdit" {
            serde_json::json!({"notebook_path": path})
        } else {
            serde_json::json!({"path": path})
        };
        let call = ToolCall {
            id: id.to_owned(),
            kind: "function".into(),
            function: FunctionCall {
                name: name.into(),
                arguments: arguments.to_string(),
            },
        };
        Message {
            role: "assistant".into(),
            content: None,
            content_blocks: None,
            tool_calls: Some(vec![call]),
            tool_call_id: None,
        }
    }

    fn reply(id: &str, text: &str) -> Message {
        Message {
            role: "tool".into(),
            content: Some(text.into()),
            content_blocks: None,
            tool_calls: None,
            tool_call_id: Some(id.into()),
        }
    }

    #[test]
    fn only_completed_successful_file_reads_are_reported_once() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("a.rs"), "test").unwrap();
        std::fs::write(root.path().join("b.rs"), "test").unwrap();
        let messages = vec![
            tool("a", "read_file", "a.rs"),
            tool("b", "read_file", "missing.rs"),
            tool("c", "read_file", "b.rs"),
            reply("b", "ERROR: denied"),
            reply("c", "contents"),
            reply("a", "contents"),
            tool("d", "read_file", "a.rs"),
            reply("d", "contents"),
        ];
        assert_eq!(paths(&messages, root.path()), vec!["a.rs", "b.rs"]);
        assert!(render(&messages, root.path()).contains("Files in context:\na.rs\nb.rs"));
    }

    #[test]
    fn successful_writes_and_edits_update_recent_file_order_after_resume() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("existing.rs"), "old").unwrap();
        let messages = vec![
            tool("a", "Read", "existing.rs"),
            reply("a", "old"),
            tool("b", "Write", "created.rs"),
            reply("b", "Wrote 3 bytes"),
            tool("c", "Edit", "existing.rs"),
            reply("c", "Replaced 1 occurrence"),
            tool("d", "NotebookEdit", "notes.ipynb"),
            reply("d", "ERROR: denied"),
        ];
        // The file can be removed between a successful tool result and
        // replaying the transcript; the cache key still existed then.
        assert_eq!(
            paths(&messages, root.path()),
            vec!["existing.rs", "created.rs"]
        );
    }

    #[test]
    fn file_list_is_bounded_and_uses_most_recent_access() {
        let root = tempfile::tempdir().unwrap();
        let mut messages = Vec::new();
        for index in 0..105 {
            let id = index.to_string();
            messages.push(tool(&id, "write_file", &format!("{index}.rs")));
            messages.push(reply(&id, "Wrote 1 byte"));
        }
        let files = paths(&messages, root.path());
        assert_eq!(files.len(), 100);
        assert_eq!(files.first().map(String::as_str), Some("104.rs"));
        assert_eq!(files.last().map(String::as_str), Some("5.rs"));
    }

    #[test]
    fn no_read_returns_original_empty_message() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(render(&[], root.path()), "No files in context");
        assert!(paths(&[tool("unknown", "Read", "/nonexistent")], root.path()).is_empty());
    }
}
