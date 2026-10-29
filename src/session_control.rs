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
    pub messages: usize,
    pub modified_ms: u128,
}
fn preview(messages: &[Message]) -> String {
    messages
        .iter()
        .find(|m| m.role == "user")
        .and_then(|m| m.content.as_deref())
        .unwrap_or("")
        .trim()
        .lines()
        .next()
        .unwrap_or("")
        .chars()
        .take(80)
        .collect()
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
        let title = session
            .custom_title
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| {
                if fallback.is_empty() {
                    "Untitled conversation".into()
                } else {
                    fallback
                }
            });
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
            messages: session.messages.len(),
            modified_ms,
        });
    }
    entries.sort_by(|a, b| b.modified_ms.cmp(&a.modified_ms).then(a.id.cmp(&b.id)));
    Ok(entries)
}
pub fn search(root: &Path, query: &str) -> Result<Vec<SessionItem>> {
    let matches = list(root)?;
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Ok(matches);
    }
    Ok(matches
        .into_iter()
        .filter(|item| {
            item.id.to_string().contains(&query) || item.title.to_lowercase().contains(&query)
        })
        .collect())
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
    let old = session.custom_title.replace(title.to_owned());
    if let Err(err) = session.save(root) {
        session.custom_title = old;
        return Err(err).context("Failed to persist session title");
    }
    Ok(format!("Session renamed to: {title}"))
}
pub fn rename_saved(root: &Path, id: &str, title: &str) -> Result<String> {
    let mut session = Session::load(root, id)?;
    rename(&mut session, root, title)
}
pub fn first_prompt(messages: &[Message]) -> String {
    let first = preview(messages);
    if first.chars().count() > 50 {
        first.chars().take(49).collect::<String>() + "…"
    } else {
        first
    }
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
    let name = if label.is_empty() {
        "conversation".to_owned()
    } else {
        label
    };
    PathBuf::from(format!("{name}-{}.txt", session.id))
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
        if let Some(content) = &message.content {
            lines.push(content.clone());
        }
        if let Some(calls) = &message.tool_calls {
            for call in calls {
                lines.push(format!(
                    "Tool call [{}] {}:\n{}",
                    call.id, call.function.name, call.function.arguments
                ));
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
