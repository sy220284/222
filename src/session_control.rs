//! Workspace-local conversation history, persisted titles and plaintext export.
use crate::{protocol::Message, session::Session};
use anyhow::{bail, Context, Result};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SessionItem {
    pub id: Uuid,
    pub title: String,
    pub tag: Option<String>,
    pub messages: usize,
    pub modified_ms: u128,
}
impl SessionItem {
    pub fn display_title(&self) -> String {
        match self.tag.as_deref() {
            Some(tag) if !tag.is_empty() => format!("{} #{tag}", self.title),
            _ => self.title.clone(),
        }
    }
}
fn preview(messages: &[Message]) -> String {
    // 2.1.88 displays the first human prompt even when its content is a
    // multimodal array. Extract visible text; never use tool output as a title.
    let Some(first) = messages.iter().find(|m| m.role == "user") else {
        return String::new();
    };
    let text = if let Some(text) = first.content.as_deref() {
        text.to_owned()
    } else {
        first
            .content_blocks
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .filter_map(|block| {
                block
                    .as_str()
                    .or_else(|| block.get("text").and_then(serde_json::Value::as_str))
            })
            .collect::<Vec<_>>()
            .join(" ")
    };
    text.trim()
        .lines()
        .next()
        .unwrap_or("")
        .chars()
        .take(80)
        .collect()
}
/// The original 2.1.88 sessionTitle.extractConversationText uses the
/// recent tail of user/assistant text for model-generated names. Keep text
/// blocks from multimodal messages, but never feed tool output to the model.
pub fn conversation_text_for_name(messages: &[Message]) -> String {
    let mut parts = Vec::new();
    for message in messages {
        if !matches!(message.role.as_str(), "user" | "assistant") {
            continue;
        }
        if let Some(content) = message.content.as_deref() {
            parts.push(content);
        } else if let Some(blocks) = message.content_blocks.as_deref() {
            for block in blocks {
                if block.get("type").and_then(serde_json::Value::as_str) == Some("text") {
                    if let Some(text) = block.get("text").and_then(serde_json::Value::as_str) {
                        parts.push(text);
                    }
                }
            }
        }
    }
    let text = parts.join("\n");
    // JS slices the last 1,000 UTF-16 units; preserve valid Rust Unicode
    // scalar boundaries rather than splitting an emoji's surrogate pair.
    let mut used = 0;
    let mut start = text.len();
    for (offset, ch) in text.char_indices().rev() {
        if used + ch.len_utf16() > 1_000 {
            break;
        }
        used += ch.len_utf16();
        start = offset;
    }
    text[start..].to_owned()
}

pub fn list(root: &Path) -> Result<Vec<SessionItem>> {
    let root = root.canonicalize().context("Workspace unavailable")?;
    let directory = root.join(".atlas/sessions");
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let real = directory.canonicalize()?;
    if !real.starts_with(&root) {
        bail!("Session directory escapes workspace")
    }
    let mut entries = Vec::new();
    for entry in fs::read_dir(real)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let filename = entry.file_name();
        let Some(id) = filename
            .to_str()
            .and_then(|s| s.strip_suffix(".json"))
            .and_then(|s| Uuid::parse_str(s).ok())
        else {
            continue;
        };
        let meta = entry.metadata()?;
        if meta.len() > 50 * 1024 * 1024 {
            continue;
        }
        let session = match Session::load(&root, &id.to_string()) {
            Ok(s) => s,
            Err(err) => {
                eprintln!("Skipping invalid session {id}: {err:#}");
                continue;
            }
        };
        let fallback = preview(&session.messages);
        // log.ts:getLogDisplayTitle skips autonomous <tick> prompts and
        // command-only XML. Strip before choosing a fallback, rather than
        // displaying raw markup or an empty session title.
        let autonomous = fallback.starts_with("<tick>");
        let visible_prompt = crate::portable_ui_runtime::strip_display_tags_allow_empty(&fallback);
        let fallback_title = if autonomous {
            "Autonomous session".to_owned()
        } else if visible_prompt.is_empty() {
            id.to_string().chars().take(8).collect()
        } else {
            visible_prompt
        };
        // getLogDisplayTitle prioritizes teammate name, explicit title,
        // summary and a visible first prompt in that order.
        let title = session
            .agent_name
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                session
                    .custom_title
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
            })
            .or_else(|| {
                session
                    .summary
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
            })
            .map(str::to_owned)
            .unwrap_or(fallback_title);
        let modified_ms = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|t| t.as_millis())
            .unwrap_or(0);
        let title = crate::portable_ui_runtime::strip_display_tags(&title);
        entries.push(SessionItem {
            id,
            title,
            tag: session.tag,
            messages: session.messages.len(),
            modified_ms,
        });
    }
    entries.sort_by(|a, b| b.modified_ms.cmp(&a.modified_ms).then(a.id.cmp(&b.id)));
    Ok(entries)
}
// 2.1.88 utils/agenticSessionSearch.ts scans the first and last 50 messages,
// skips non-conversation roles and searches only the first 2000 text characters.
// This is the deterministic local pre-filter; semantic model ranking remains
// a separate feature and must not be claimed as implemented here.
fn searchable_transcript(messages: &[Message]) -> String {
    let mut parts = Vec::new();
    let selected: Vec<&Message> = if messages.len() <= 100 {
        messages.iter().collect()
    } else {
        messages
            .iter()
            .take(50)
            .chain(messages.iter().skip(messages.len() - 50))
            .collect()
    };
    for message in selected {
        if message.role != "user" && message.role != "assistant" {
            continue;
        }
        if let Some(text) = message.content.as_deref().filter(|s| !s.is_empty()) {
            parts.push(text.to_owned());
        } else if let Some(blocks) = &message.content_blocks {
            let text = blocks
                .iter()
                .filter_map(|block| {
                    block
                        .as_str()
                        .or_else(|| block.get("text").and_then(|v| v.as_str()))
                })
                .collect::<Vec<_>>()
                .join(" ");
            if !text.is_empty() {
                parts.push(text);
            }
        }
    }
    let text = parts
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    // Original extractTranscript limits 2,000 UTF-16 units and marks
    // truncation with an ellipsis. Keep Rust Unicode boundaries valid.
    let mut used = 0;
    for (offset, ch) in text.char_indices() {
        if used + ch.len_utf16() > 2_000 {
            return format!("{}…", &text[..offset]);
        }
        used += ch.len_utf16();
    }
    text
}

pub fn search(root: &Path, query: &str) -> Result<Vec<SessionItem>> {
    let matches = list(root)?;
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Ok(matches);
    }
    // A single # prefixes an explicit tag lookup; a bare # must not
    // match every tagged conversation through str::contains("").
    let tag_query = query.strip_prefix('#').unwrap_or(&query);
    let mut ranked = Vec::new();
    for item in matches {
        let tag = item.tag.as_deref().unwrap_or("").to_lowercase();
        let rank = if !tag_query.is_empty() && tag == tag_query {
            Some(0u8)
        } else if !tag_query.is_empty() && tag.contains(tag_query) {
            Some(1)
        } else if item.title.to_lowercase().contains(&query) {
            Some(2)
        } else if item.id.to_string().contains(&query) {
            Some(3)
        } else {
            // The original searches the summary and the first/last transcript
            // excerpts as well as the title and user tag.
            Session::load(root, &item.id.to_string())
                .ok()
                .and_then(|session| {
                    if session
                        .summary
                        .as_deref()
                        .is_some_and(|s| s.to_lowercase().contains(&query))
                    {
                        Some(4)
                    } else if searchable_transcript(&session.messages)
                        .to_lowercase()
                        .contains(&query)
                    {
                        Some(5)
                    } else {
                        None
                    }
                })
        };
        if let Some(rank) = rank {
            ranked.push((rank, item));
        }
    }
    ranked.sort_by(|(rank_a, a), (rank_b, b)| {
        rank_a.cmp(rank_b).then(b.modified_ms.cmp(&a.modified_ms))
    });
    Ok(ranked.into_iter().map(|(_, item)| item).collect())
}

/// Resolve a UUID, exact title, or unambiguous title/UUID substring. Do not
/// silently resume the wrong conversation when several titles match.
pub fn resolve(root: &Path, selector: &str) -> Result<Session> {
    let selector = selector.trim();
    if let Ok(id) = Uuid::parse_str(selector) {
        return Session::load(root, &id.to_string());
    }
    let matches = search(root, selector)?;
    if matches.is_empty() {
        bail!("Session {selector} was not found")
    }
    let exact = matches
        .iter()
        .filter(|item| item.title.eq_ignore_ascii_case(selector))
        .collect::<Vec<_>>();
    let selected = if exact.len() == 1 {
        exact[0]
    } else if matches.len() == 1 {
        &matches[0]
    } else {
        bail!(
            "Found {} sessions matching '{}'. Please choose a specific UUID.",
            matches.len(),
            selector
        );
    };
    Session::load(root, &selected.id.to_string())
}
pub fn rename(session: &mut Session, root: &Path, title: &str) -> Result<String> {
    let title = title.trim();
    if title.is_empty() {
        bail!("Usage: /rename <name>")
    }
    if session.team_name.is_some() && session.agent_name.is_some() {
        bail!("Cannot rename: This session is a swarm teammate. Teammate names are set by the team leader.");
    }
    // rename.ts saves both the user-visible custom title and the session's
    // agent name. Session listings prefer agent_name; updating only the
    // custom title leaves an old name visible after a successful /rename.
    let old_title = session.custom_title.replace(title.to_owned());
    let old_agent_name = session.agent_name.replace(title.to_owned());
    if let Err(err) = session.save(root) {
        session.custom_title = old_title;
        session.agent_name = old_agent_name;
        return Err(err).context("Failed to persist session title");
    }
    Ok(format!("Session renamed to: {title}"))
}
pub fn rename_saved(root: &Path, id: &str, title: &str) -> Result<String> {
    let mut session = Session::load(root, id)?;
    rename(&mut session, root, title)
}
pub fn first_prompt(messages: &[Message]) -> String {
    // commands/export/export.tsx uses the first user message's first text
    // block, not the concatenation of every block in a multimodal prompt.
    let Some(message) = messages.iter().find(|message| message.role == "user") else {
        return String::new();
    };
    let text = message.content.clone().or_else(|| {
        message.content_blocks.as_ref().and_then(|blocks| {
            blocks.iter().find_map(|block| {
                if block.get("type").and_then(|value| value.as_str()) == Some("text") {
                    block
                        .get("text")
                        .and_then(|value| value.as_str())
                        .map(str::to_owned)
                } else {
                    None
                }
            })
        })
    });
    let text = text.unwrap_or_default();
    let line = text.trim().lines().next().unwrap_or("");
    let mut result = String::new();
    let mut length = 0;
    for ch in line.chars() {
        if length + ch.len_utf16() > 50 {
            break;
        }
        result.push(ch);
        length += ch.len_utf16();
    }
    if line.encode_utf16().count() > 50 {
        while result.encode_utf16().count() > 49 {
            result.pop();
        }
        result.push('…');
    }
    result
}
pub fn sanitize_filename(input: &str) -> String {
    let cleaned = input
        .to_ascii_lowercase()
        .chars()
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c.is_whitespace() || *c == '-')
        .collect::<String>();
    let mut out = String::new();
    let mut pending_dash = false;
    for c in cleaned.chars() {
        if c.is_whitespace() || c == '-' {
            if !out.is_empty() {
                pending_dash = true;
            }
        } else {
            if pending_dash {
                out.push('-');
            }
            out.push(c);
            pending_dash = false;
        }
    }
    out
}
fn filename(session: &Session, requested: Option<&str>) -> PathBuf {
    if let Some(p) = requested.filter(|x| !x.trim().is_empty()) {
        let mut filename = PathBuf::from(p.trim());
        if !p.trim().ends_with(".txt") {
            filename.set_extension("txt");
        }
        return filename;
    }
    let label = sanitize_filename(&first_prompt(&session.messages));
    // The original export dialog proposes a local-time timestamp before
    // the sanitized first prompt; a UUID-based name is not source-equivalent.
    let timestamp = chrono::Local::now().format("%Y-%m-%d-%H%M%S");
    if label.is_empty() {
        PathBuf::from(format!("conversation-{timestamp}.txt"))
    } else {
        PathBuf::from(format!("{timestamp}-{label}.txt"))
    }
}
// The 2.1.88 exportRenderer renders normalized multimodal message blocks
// before stripping terminal styling. Preserve all visible text blocks in a
// plaintext export, without dumping binary image/audio payloads.
fn render_export_block(block: &serde_json::Value) -> Option<String> {
    if let Some(text) = block.as_str() {
        return Some(text.to_owned());
    }
    let kind = block.get("type").and_then(serde_json::Value::as_str);
    match kind {
        Some("text" | "input_text" | "output_text") => block
            .get("text")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        Some("tool_use") => {
            let id = block
                .get("id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            let name = block
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("tool");
            let arguments = block
                .get("input")
                .map(serde_json::Value::to_string)
                .unwrap_or_else(|| "{}".to_owned());
            Some(format!("Tool call [{id}] {name}:\n{arguments}"))
        }
        Some("tool_result") => {
            let id = block
                .get("tool_use_id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            let content = block.get("content")?;
            let output = match content {
                serde_json::Value::String(text) => text.to_owned(),
                serde_json::Value::Array(blocks) => blocks
                    .iter()
                    .filter_map(render_export_block)
                    .collect::<Vec<_>>()
                    .join("\n"),
                other => render_export_block(other).unwrap_or_default(),
            };
            Some(format!("Tool result [{id}]:\n{output}"))
        }
        Some("image" | "image_url") => Some("[Image]".to_owned()),
        Some("document") => Some("[Document]".to_owned()),
        Some("audio" | "input_audio" | "output_audio") => Some("[Audio]".to_owned()),
        Some("video") => Some("[Video]".to_owned()),
        Some("thinking" | "redacted_thinking") => None,
        _ => block
            .get("text")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
    }
}
pub fn render(session: &Session) -> String {
    let mut lines = Vec::new();
    for message in &session.messages {
        let role = match message.role.as_str() {
            "user" => "User",
            "assistant" => "Assistant",
            "system" => "System",
            "tool" => "Tool result",
            other => other,
        };
        lines.push(match &message.tool_call_id {
            Some(id) => format!("{role} [{id}]:"),
            None => format!("{role}:"),
        });
        // Content blocks are the lossless representation of SDK multimodal
        // messages. The flattened content field may omit later text blocks.
        if let Some(blocks) = message.content_blocks.as_ref().filter(|b| !b.is_empty()) {
            // Some providers supply the ordinary text in content while
            // placing only attachments or tool calls in content_blocks.
            // Keep that text, but do not duplicate content already present
            // as a text block or a structured tool result.
            let has_text_or_result = blocks.iter().any(|block| {
                block.as_str().is_some()
                    || matches!(
                        block.get("type").and_then(serde_json::Value::as_str),
                        Some("text" | "input_text" | "output_text" | "tool_result")
                    )
            });
            if !has_text_or_result {
                if let Some(content) = &message.content {
                    lines.push(content.clone());
                }
            }
            for block in blocks {
                if let Some(text) = render_export_block(block) {
                    lines.push(text);
                }
            }
        } else if let Some(content) = &message.content {
            lines.push(content.clone());
        }
        if let Some(calls) = &message.tool_calls {
            for call in calls {
                // A tool_use content block already occupies its original
                // position in the transcript; do not export it twice.
                let already_rendered = message.content_blocks.as_ref().is_some_and(|blocks| {
                    blocks.iter().any(|block| {
                        block.get("type").and_then(serde_json::Value::as_str) == Some("tool_use")
                            && block.get("id").and_then(serde_json::Value::as_str)
                                == Some(call.id.as_str())
                    })
                });
                if !already_rendered {
                    lines.push(format!(
                        "Tool call [{}] {}:\n{}",
                        call.id, call.function.name, call.function.arguments
                    ));
                }
            }
        }
        lines.push(String::new());
    }
    lines.join("\n")
}
pub fn export(session: &Session, root: &Path, path: Option<&str>) -> Result<PathBuf> {
    let root = root.canonicalize().context("Workspace unavailable")?;
    let requested = filename(session, path);
    let target = if requested.is_absolute() {
        requested
    } else {
        root.join(requested)
    };
    let parent = target
        .parent()
        .context("Invalid export path")?
        .canonicalize()
        .context("Export parent directory unavailable")?;
    if !parent.starts_with(&root) {
        bail!("Export path leaves workspace")
    }
    let final_path = parent.join(target.file_name().context("Invalid export filename")?);
    if final_path
        .symlink_metadata()
        .is_ok_and(|m| m.file_type().is_symlink())
    {
        bail!("Export destination must not be a symlink")
    }
    let tmp = parent.join(format!(
        ".{}-{}.tmp",
        final_path.file_name().unwrap().to_string_lossy(),
        Uuid::new_v4()
    ));
    let result = (|| -> Result<()> {
        let mut file = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        file.write_all(render(session).as_bytes())?;
        file.sync_all()?;
        fs::rename(&tmp, &final_path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result.with_context(|| format!("Export failed: {}", final_path.display()))?;
    Ok(final_path)
}
pub fn export_saved(root: &Path, id: &str, path: Option<&str>) -> Result<PathBuf> {
    let session = Session::load(root, id)?;
    export(&session, root, path)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{FunctionCall, ToolCall};
    #[test]
    fn generated_name_context_uses_recent_text_and_excludes_tool_results() {
        let messages = vec![
            Message::text("user", "first request"),
            Message::text("tool", "secret tool output"),
            Message {
                role: "assistant".into(),
                content: None,
                content_blocks: Some(vec![
                    serde_json::json!({"type":"text","text":"latest fix"}),
                    serde_json::json!({"type":"image","data":"image bytes"}),
                    serde_json::json!({"type":"text","text":"next step"}),
                ]),
                tool_calls: None,
                tool_call_id: None,
            },
        ];
        assert_eq!(
            conversation_text_for_name(&messages),
            "first request\nlatest fix\nnext step"
        );
        assert!(conversation_text_for_name(&[Message::text("tool", "hidden")]).is_empty());

        let long = vec![
            Message::text("user", "old context ".repeat(200)),
            Message::text("assistant", "newest task"),
        ];
        let text = conversation_text_for_name(&long);
        assert!(text.ends_with("newest task"));
        assert!(text.encode_utf16().count() <= 1_000);
        assert!(!text.starts_with("old context old context old context old context old context"));

        let unicode =
            conversation_text_for_name(&[Message::text("user", format!("{}🦀", "x".repeat(999)))]);
        assert_eq!(unicode.encode_utf16().count(), 1_000);
        assert!(unicode.ends_with("🦀"));
    }

    #[test]
    fn rename_list_export_and_restore_full_conversation() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let mut session = Session::fresh();
        session
            .messages
            .push(Message::text("user", "Fix login\nsecond line"));
        session.messages.push(Message {
            role: "assistant".into(),
            content: Some("Check source".into()),
            tool_calls: Some(vec![ToolCall {
                id: "call-1".into(),
                kind: "function".into(),
                function: FunctionCall {
                    name: "read_file".into(),
                    arguments: r#"{"path":"main.rs"}"#.into(),
                },
            }]),
            tool_call_id: None,
            content_blocks: None,
        });
        session.messages.push(Message::tool_result(
            &ToolCall {
                id: "call-1".into(),
                kind: "function".into(),
                function: FunctionCall {
                    name: "read_file".into(),
                    arguments: "{}".into(),
                },
            },
            "source file".into(),
        ));
        session.save(root).unwrap();
        assert_eq!(list(root).unwrap()[0].title, "Fix login");
        assert_eq!(
            rename(&mut session, root, "  Active project  ").unwrap(),
            "Session renamed to: Active project"
        );
        assert_eq!(
            Session::load(root, &session.id.to_string())
                .unwrap()
                .custom_title
                .as_deref(),
            Some("Active project")
        );
        assert_eq!(list(root).unwrap()[0].title, "Active project");
        let output = export(&session, root, Some("history.md")).unwrap();
        assert_eq!(output, root.join("history.txt"));
        let rendered = fs::read_to_string(output).unwrap();
        for expected in [
            "User:\nFix login",
            "Assistant:\nCheck source",
            "Tool call [call-1] read_file:",
            "Tool result [call-1]:\nsource file",
        ] {
            assert!(rendered.contains(expected), "{expected}");
        }
        assert_eq!(sanitize_filename("Fix_abc  UI---Test!"), "fixabc-ui-test");
        assert_eq!(first_prompt(&session.messages), "Fix login");
        session.team_name = Some("team".into());
        session.agent_name = Some("worker".into());
        assert!(rename(&mut session, root, "not allowed").is_err());
    }
    #[test]
    fn session_search_is_case_insensitive_and_ambiguous_queries_fail() {
        let root = tempfile::tempdir().unwrap();
        let mut first = Session::fresh();
        first
            .messages
            .push(Message::text("user", "Fix parser behavior"));
        first.save(root.path()).unwrap();
        let mut second = Session::fresh();
        second
            .messages
            .push(Message::text("user", "Fix agent logic"));
        second.save(root.path()).unwrap();
        assert_eq!(search(root.path(), "PARSER").unwrap().len(), 1);
        assert!(resolve(root.path(), "Fix").is_err());
        assert_eq!(
            resolve(root.path(), "Fix parser behavior").unwrap().id,
            first.id
        );
        assert_eq!(
            resolve(root.path(), &second.id.to_string()).unwrap().id,
            second.id
        );
        assert!(resolve(root.path(), "nonexistent").is_err());
    }
    #[test]
    fn session_search_uses_original_tag_summary_and_transcript_pre_filter() {
        let root = tempfile::tempdir().unwrap();
        let mut tagged = Session::fresh();
        tagged.tag = Some("release".into());
        tagged.messages.push(Message::text("user", "unrelated"));
        tagged.save(root.path()).unwrap();

        let mut summarized = Session::fresh();
        summarized.summary = Some("Release preparation checklist".into());
        summarized
            .messages
            .push(Message::text("user", "other topic"));
        summarized.save(root.path()).unwrap();

        let mut transcript = Session::fresh();
        transcript.messages.push(Message {
            role: "user".into(),
            content: None,
            content_blocks: Some(vec![serde_json::json!({
                "type": "text", "text": "Review release readiness"
            })]),
            tool_calls: None,
            tool_call_id: None,
        });
        transcript.save(root.path()).unwrap();

        let mut tool_only = Session::fresh();
        tool_only
            .messages
            .push(Message::text("tool", "release secret output"));
        tool_only.save(root.path()).unwrap();

        let found = search(root.path(), "release").unwrap();
        // Both summary and transcript become visible titles containing
        // "release" (the same rank). Their modification timestamps can be
        // equal at filesystem resolution; creation order is not a search
        // ordering contract. Assert rank and membership instead.
        assert_eq!(found.len(), 3);
        assert_eq!(found[0].id, tagged.id);
        let tied_titles = [found[1].id, found[2].id];
        assert!(tied_titles.contains(&summarized.id));
        assert!(tied_titles.contains(&transcript.id));
        assert_eq!(search(root.path(), "#release").unwrap()[0].id, tagged.id);
        assert!(search(root.path(), "secret").unwrap().is_empty());
    }

    #[test]
    fn transcript_search_scans_both_ends_without_leaking_middle_or_tool_output() {
        let mut messages = vec![Message::text("user", "opening match")];
        messages.extend((0..110).map(|i| Message::text("user", format!("middle {i}"))));
        messages.push(Message::text("assistant", "closing match"));
        let text = searchable_transcript(&messages);
        assert!(text.contains("opening match"));
        assert!(text.contains("closing match"));
        assert!(!text.contains("middle 55"));
        assert!(text.chars().count() <= 2000);
        let short = [
            Message::text("user", "first"),
            Message::text("assistant", "last"),
        ];
        assert_eq!(searchable_transcript(&short), "first last");
    }

    #[test]
    fn transcript_search_uses_original_utf16_limit_and_truncation_marker() {
        let text = searchable_transcript(&[Message::text("user", "x".repeat(2_100))]);
        assert_eq!(text, format!("{}…", "x".repeat(2_000)));
        let unicode = searchable_transcript(&[Message::text(
            "assistant",
            format!("{}🦀tail", "x".repeat(1_999)),
        )]);
        assert_eq!(unicode, format!("{}…", "x".repeat(1_999)));
    }

    #[cfg(unix)]
    #[test]
    fn refuses_workspace_escape_and_symlink_exports() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let session = Session::fresh();
        let destination = other.path().join("secret.txt");
        fs::write(&destination, "keep").unwrap();
        symlink(&destination, root.path().join("copy.txt")).unwrap();
        assert!(export(&session, root.path(), Some("copy.txt")).is_err());
        assert!(export(&session, root.path(), Some(destination.to_str().unwrap())).is_err());
        assert_eq!(fs::read_to_string(destination).unwrap(), "keep");
    }
}
