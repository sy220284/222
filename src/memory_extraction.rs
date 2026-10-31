//! Opt-in, provider-neutral extraction of durable project memories.
//! The model produces data only; it receives no tools or filesystem authority.
use crate::{
    config::ProviderProfile,
    protocol::{Message, ModelRequest, TokenUsage},
    provider::ModelProvider,
};
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::{
    collections::HashSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

const MAX_ITEMS: usize = 4;
const MAX_BODY: usize = 12_000;
const MAX_INPUT: usize = 32_000;
const MAX_MANIFEST: usize = 20_000;

pub fn enabled() -> bool {
    crate::env_runtime::is_env_truthy(
        std::env::var("ATLAS_AUTO_MEMORY_EXTRACTION")
            .ok()
            .as_deref(),
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Item {
    filename: String,
    #[serde(rename = "type")]
    kind: String,
    description: String,
    content: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Output {
    memories: Vec<Item>,
}

pub struct Outcome {
    pub cursor: usize,
    pub files_written: usize,
    pub usage: Option<TokenUsage>,
}

fn memory_dir(workspace: &Path) -> Result<PathBuf> {
    let workspace = workspace.canonicalize().context("Resolve workspace")?;
    let atlas = workspace.join(".atlas");
    fs::create_dir_all(&atlas)?;
    let atlas = atlas.canonicalize().context("Resolve .atlas")?;
    if !atlas.starts_with(&workspace) {
        bail!("Memory directory escapes workspace");
    }
    let dir = atlas.join("memory");
    fs::create_dir_all(&dir)?;
    let dir = dir.canonicalize().context("Resolve memory directory")?;
    if !dir.starts_with(&atlas) {
        bail!("Memory directory escapes workspace");
    }
    Ok(dir)
}

fn topic(dir: &Path, filename: &str) -> Result<PathBuf> {
    let valid = filename.strip_suffix(".md").is_some_and(|stem| {
        !stem.is_empty()
            && stem.len() <= 60
            && stem
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
    });
    if !valid || filename == "MEMORY.md" {
        bail!("Invalid memory topic filename");
    }
    let path = dir.join(filename);
    if path
        .symlink_metadata()
        .is_ok_and(|m| m.file_type().is_symlink() || !m.is_file())
    {
        bail!("Memory topic is not a regular file");
    }
    Ok(path)
}

fn manifest(dir: &Path) -> Result<(String, HashSet<String>)> {
    let mut names = fs::read_dir(dir)?
        .map(|e| e.map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect::<std::io::Result<Vec<String>>>()?;
    names.sort();
    let mut existing = String::new();
    let mut visible = HashSet::new();
    for name in names {
        if name == "MEMORY.md" {
            continue;
        }
        let Ok(path) = topic(dir, &name) else {
            continue;
        };
        let body = fs::read_to_string(&path).context("Read memory topic")?;
        let fragment = format!("\n### {name}\n{body}\n");
        if fragment.len() > MAX_MANIFEST.saturating_sub(existing.len()) {
            continue;
        }
        existing.push_str(&fragment);
        visible.insert(name);
    }
    Ok((existing, visible))
}

fn atomic_write(path: &Path, content: &[u8]) -> Result<()> {
    let tmp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let mut handle = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)?;
    let result = (|| -> Result<()> {
        handle.write_all(content)?;
        handle.sync_all()?;
        fs::rename(&tmp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

fn save(dir: &Path, items: &[Item], visible: &HashSet<String>) -> Result<usize> {
    if items.len() > MAX_ITEMS {
        bail!("Too many extracted memories");
    }
    let mut written = Vec::new();
    let mut seen = HashSet::new();
    for item in items {
        let path = topic(dir, &item.filename)?;
        if !seen.insert(item.filename.as_str()) {
            bail!("Repeated memory filename");
        }
        if path.exists() && !visible.contains(&item.filename) {
            bail!("Refusing to overwrite a memory excluded from context");
        }
        if !matches!(
            item.kind.as_str(),
            "user" | "feedback" | "project" | "reference"
        ) {
            bail!("Unsupported memory type");
        }
        if item.description.trim().is_empty()
            || item.description.len() > 180
            || item.description.chars().any(char::is_control)
        {
            bail!("Invalid memory description");
        }
        if item.content.trim().is_empty() || item.content.len() > MAX_BODY {
            bail!("Invalid memory body");
        }
        let desc = serde_json::to_string(item.description.trim())?;
        let body = format!(
            "---\ndescription: {desc}\ntype: {}\n---\n{}\n",
            item.kind,
            item.content.trim()
        );
        written.push((path, body));
    }
    let index_path = dir.join("MEMORY.md");
    if index_path
        .symlink_metadata()
        .is_ok_and(|m| m.file_type().is_symlink() || !m.is_file())
    {
        bail!("Unsafe memory index");
    }
    let current = if index_path.exists() {
        fs::read_to_string(&index_path)?
    } else {
        String::new()
    };
    let mut index = current.clone();
    for item in items {
        if !index.contains(&format!("({})", item.filename)) {
            if !index.is_empty() && !index.ends_with('\n') {
                index.push('\n');
            }
            let hook = item
                .description
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            index.push_str(&format!(
                "- [{}]({}) — {}\n",
                item.filename.trim_end_matches(".md"),
                item.filename,
                hook
            ));
        }
    }
    if index.lines().count() > 200 || index.len() > 25_000 {
        bail!("Memory index capacity exceeded");
    }
    for (path, body) in &written {
        atomic_write(path, body.as_bytes())?;
    }
    if index != current {
        atomic_write(&index_path, index.as_bytes())?;
    }
    Ok(written.len())
}

fn wrote_memory(messages: &[Message], from: usize, workspace: &Path) -> bool {
    let dir = workspace.join(".atlas/memory");
    messages.iter().skip(from).any(|msg| {
        msg.tool_calls.as_ref().is_some_and(|calls| {
            calls.iter().any(|call| {
                if !matches!(
                    call.function.name.as_str(),
                    "write_file" | "edit_file" | "Write" | "Edit"
                ) {
                    return false;
                }
                let Ok(args) = serde_json::from_str::<serde_json::Value>(&call.function.arguments)
                else {
                    return false;
                };
                let Some(path) = args
                    .get("path")
                    .or_else(|| args.get("file_path"))
                    .and_then(|value| value.as_str())
                else {
                    return false;
                };
                let raw = Path::new(path);
                let path = if raw.is_absolute() {
                    raw.to_path_buf()
                } else {
                    workspace.join(raw)
                };
                path.parent()
                    .and_then(|p| p.canonicalize().ok())
                    .is_some_and(|p| p.starts_with(&dir))
            })
        })
    })
}

pub async fn extract(
    workspace: &Path,
    messages: &[Message],
    cursor: usize,
    main_system: &str,
    provider: &dyn ModelProvider,
    profile: &ProviderProfile,
) -> Result<Outcome> {
    let dir = memory_dir(workspace)?;
    let from = if cursor > messages.len() { 0 } else { cursor };
    let end = messages.len();
    if from == end || wrote_memory(messages, from, workspace) {
        return Ok(Outcome {
            cursor: end,
            files_written: 0,
            usage: None,
        });
    }
    let mut new_messages = Vec::new();
    let mut bytes = 0;
    for msg in messages[from..].iter().rev() {
        if !matches!(msg.role.as_str(), "user" | "assistant") {
            continue;
        }
        let text = msg.visible_text();
        if text.is_empty() {
            continue;
        }
        if bytes + text.len() > MAX_INPUT {
            break;
        }
        bytes += text.len();
        new_messages.push(Message::text(&msg.role, text));
    }
    if new_messages.is_empty() {
        return Ok(Outcome {
            cursor: end,
            files_written: 0,
            usage: None,
        });
    }
    new_messages.reverse();
    let (context, visible) = manifest(&dir)?;
    let system = format!(
        r#"{main_system}

# Memory extraction (source: services/extractMemories/prompts.ts)
You are extracting durable project memory from only the new conversation messages.
Persist user preferences, constructive feedback, project decisions and reference
pointers that cannot be reconstructed from code/git. Never invent information.
Exclude ephemeral details, credentials, API keys and sensitive information.
Use one of four memory types: user, feedback, project, reference.
Organize by topic. Update existing topics only if their full contents are shown
below; retain still-valid facts. Never delete a memory.
You have no tools. Return ONLY JSON in the exact shape:
{{"memories":[{{"filename":"topic_name.md","type":"feedback",
"description":"one-line description","content":"complete Markdown memory"}}]}}
Return an empty array when there is nothing to save. At most {MAX_ITEMS} items.
Topic filenames must use only lowercase letters, numbers and underscores.
Existing memories (full contents within budget):
{context}"#
    );
    let response = provider
        .complete(&ModelRequest {
            model: profile.model.clone(),
            system,
            messages: new_messages,
            tools: vec![],
        })
        .await?;
    if response
        .message
        .tool_calls
        .as_ref()
        .is_some_and(|calls| !calls.is_empty())
    {
        bail!("Extraction model requested tools");
    }
    let output: Output = serde_json::from_str(&response.message.visible_text())
        .context("Invalid memory extraction model output")?;
    let files_written = save(&dir, &output.memories, &visible)?;
    Ok(Outcome {
        cursor: end,
        files_written,
        usage: response.usage,
    })
}

/// The original autoDream gate is disabled until opted in by the user.
/// Consolidation is checked at a completed turn; there is no timer.
pub fn dream_enabled() -> bool {
    crate::env_runtime::is_env_truthy(
        std::env::var("ATLAS_AUTO_DREAM").ok().as_deref(),
    )
}

/// Check the source's 24-hour + five other sessions gate. A process-level
/// lock prevents concurrent sessions consolidating the same memories.
/// The model receives no tools and every returned file is validated by save().
pub async fn dream_if_due(
    workspace: &Path,
    active_session: uuid::Uuid,
    provider: &dyn ModelProvider,
    profile: &ProviderProfile,
) -> Result<Option<(usize, Option<TokenUsage>)>> {
    use fs2::FileExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    const MIN_SECONDS: u64 = 24 * 3600;
    const MIN_SESSIONS: usize = 5;
    let dir = memory_dir(workspace)?;
    let stamp = dir.join(".dream-consolidated");
    if stamp.symlink_metadata().is_ok_and(|m| m.file_type().is_symlink() || !m.is_file()) {
        bail!("Dream timestamp must be a regular file");
    }
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let last = if stamp.exists() {
        fs::read_to_string(&stamp)?
            .trim()
            .parse::<u64>()
            .context("Invalid Dream timestamp")?
    } else {
        0
    };
    if now.saturating_sub(last) < MIN_SECONDS {
        return Ok(None);
    }

    let base = workspace.canonicalize()?.join(".atlas/sessions");
    if !base.exists() {
        return Ok(None);
    }
    if !base.canonicalize()?.starts_with(workspace.canonicalize()?) {
        bail!("Dream transcript directory escapes workspace");
    }
    let mut changed = Vec::new();
    for entry in fs::read_dir(&base)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if !entry.file_type()?.is_file() || !metadata.is_file() {
            continue;
        }
        let name = entry.file_name();
        let Some(raw) = name.to_str().and_then(|n| n.strip_suffix(".json")) else {
            continue;
        };
        let Ok(id) = uuid::Uuid::parse_str(raw) else {
            continue;
        };
        if id == active_session {
            continue;
        }
        let modified = metadata.modified()?.duration_since(UNIX_EPOCH)?.as_secs();
        if modified > last {
            changed.push((modified, id));
        }
    }
    if changed.len() < MIN_SESSIONS {
        return Ok(None);
    }
    changed.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
    changed.truncate(MIN_SESSIONS);

    let lock_path = dir.join(".dream-consolidation.lock");
    if lock_path.symlink_metadata().is_ok_and(|m| m.file_type().is_symlink() || !m.is_file()) {
        bail!("Dream lock must be a regular file");
    }
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(lock_path)?;
    match lock.try_lock_exclusive() {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(None),
        Err(error) => return Err(error.into()),
    }
    // A second session may have finished consolidation before this lock.
    if stamp.exists() {
        let actual = fs::read_to_string(&stamp)?
            .trim()
            .parse::<u64>()
            .context("Invalid Dream timestamp")?;
        if actual != last {
            return Ok(None);
        }
    }

    let (existing, visible) = manifest(&dir)?;
    let mut transcripts = String::new();
    for (_, id) in &changed {
        let session = crate::session::Session::load(workspace, &id.to_string())?;
        let mut excerpt = String::new();
        for message in session.messages.iter().rev().take(15) {
            if !matches!(message.role.as_str(), "user" | "assistant") {
                continue;
            }
            let text = message.visible_text();
            if text.is_empty() {
                continue;
            }
            let short = text.chars().take(600).collect::<String>();
            excerpt.insert_str(0, &format!("{}: {}\n", message.role, short));
        }
        if transcripts.len().saturating_add(excerpt.len()) > MAX_INPUT {
            break;
        }
        transcripts.push_str(&format!("\n## Session {id}\n{excerpt}"));
    }

    let system = format!(
        r#"# Dream: Memory Consolidation
Source: services/autoDream/consolidationPrompt.ts.
You are performing a reflective pass over existing memory files and recently
completed local sessions. Synthesize durable, organized memories so future
sessions can orient quickly. Use the established four types: user, feedback,
project, reference.

Phase 1 — Orient: review the existing topic files below; preserve valid facts.
Phase 2 — Gather: use only the supplied recent session excerpts, without
inventing or fetching unprovided transcripts.
Phase 3 — Consolidate: merge durable signals, resolve contradictions, convert
relative dates to absolute dates only when the reference date is known.
Phase 4 — Prune and index: the caller will maintain the MEMORY.md index.
Do not store credentials, user secrets, speculative inferences or duplicates.
Return ONLY JSON in the exact shape:
{{"memories":[{{"filename":"topic_name.md","type":"project",
"description":"one-line hook","content":"complete Markdown memory"}}]}}
At most {MAX_ITEMS} files; empty array when already consolidated.
The request has no tools and cannot delete files.

Existing topic contents:
{existing}

Recent local sessions:
{transcripts}"#
    );
    let response = provider
        .complete(&ModelRequest {
            model: profile.model.clone(),
            system,
            messages: vec![Message::text(
                "user",
                "Consolidate only the supplied memory and session context.",
            )],
            tools: vec![],
        })
        .await?;
    if response.message.tool_calls.as_ref().is_some_and(|calls| !calls.is_empty()) {
        bail!("Dream model requested tools");
    }
    let output: Output = serde_json::from_str(&response.message.visible_text())
        .context("Invalid Dream model output")?;
    let written = save(&dir, &output.memories, &visible)?;
    atomic_write(&stamp, now.to_string().as_bytes())?;
    Ok(Some((written, response.usage)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{FunctionCall, ModelResponse, ToolCall};
    use async_trait::async_trait;
    use std::sync::Mutex;
    struct Mock {
        output: String,
        requests: Mutex<Vec<ModelRequest>>,
    }
    #[async_trait]
    impl ModelProvider for Mock {
        async fn complete(&self, req: &ModelRequest) -> Result<ModelResponse> {
            self.requests.lock().unwrap().push(req.clone());
            Ok(ModelResponse {
                message: Message::text("assistant", &self.output),
                finish_reason: Some("stop".into()),
                usage: None,
            })
        }
    }
    fn mock(output: &str) -> Mock {
        Mock {
            output: output.into(),
            requests: Mutex::new(vec![]),
        }
    }
    fn profile() -> ProviderProfile {
        ProviderProfile {
            id: "mock".into(),
            kind: "openai-compatible".into(),
            model: "fake".into(),
            base_url: "http://127.0.0.1:1/v1".into(),
            api_key_env: None,
            stream: false,
            tuning: Default::default(),
        }
    }
    #[tokio::test]
    async fn extracts_persists_and_resumes_without_new_model_call() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let model = mock(
            r#"{"memories":[{"filename":"preferences.md","type":"feedback",
            "description":"Verify real code","content":"Run tests before completion."}]}"#,
        );
        let messages = vec![
            Message::text("user", "Please remember my workflow"),
            Message::text("assistant", "Acknowledged"),
        ];
        let first = extract(&root, &messages, 0, "Main system", &model, &profile())
            .await
            .unwrap();
        assert_eq!(first.cursor, 2);
        assert_eq!(first.files_written, 1);
        assert!(
            fs::read_to_string(root.join(".atlas/memory/preferences.md"))
                .unwrap()
                .contains("type: feedback")
        );
        assert!(fs::read_to_string(root.join(".atlas/memory/MEMORY.md"))
            .unwrap()
            .contains("(preferences.md)"));
        assert!(model.requests.lock().unwrap()[0].tools.is_empty());
        extract(
            &root,
            &messages,
            first.cursor,
            "Main system",
            &model,
            &profile(),
        )
        .await
        .unwrap();
        assert_eq!(model.requests.lock().unwrap().len(), 1);
    }
    #[tokio::test]
    async fn skips_when_main_agent_already_wrote_memory() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        fs::create_dir_all(root.join(".atlas/memory")).unwrap();
        let mut assistant = Message::text("assistant", "Memory saved");
        assistant.tool_calls = Some(vec![ToolCall {
            id: "call".into(),
            kind: "function".into(),
            function: FunctionCall {
                name: "write_file".into(),
                arguments: r#"{"path":".atlas/memory/preferences.md"}"#.into(),
            },
        }]);
        let model = mock("invalid");
        let result = extract(
            &root,
            &[Message::text("user", "Remember this"), assistant],
            0,
            "system",
            &model,
            &profile(),
        )
        .await
        .unwrap();
        assert_eq!(result.cursor, 2);
        assert!(model.requests.lock().unwrap().is_empty());
    }
    #[tokio::test]
    async fn invalid_destination_prevents_partial_writes() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let model = mock(
            r#"{"memories":[{"filename":"valid.md","type":"user",
        "description":"safe","content":"Good"},{"filename":"../leak.md","type":"user",
        "description":"bad","content":"Bad"}]}"#,
        );
        assert!(extract(
            &root,
            &[Message::text("user", "save")],
            0,
            "sys",
            &model,
            &profile()
        )
        .await
        .is_err());
        assert!(!root.join(".atlas/memory/valid.md").exists());
        assert!(!root.join("leak.md").exists());
    }
    #[tokio::test]
    async fn dream_requires_five_other_sessions_and_runs_only_once_per_day() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path().canonicalize().unwrap();
        let active = uuid::Uuid::new_v4();
        for n in 0..4 {
            let mut session = crate::session::Session::fresh();
            session.messages.push(Message::text("user", format!("Decision {n}")));
            session.save(&root).unwrap();
        }
        let model = mock(r#"{"memories":[{"filename":"consolidated.md","type":"project",
            "description":"Stable project decision","content":"Project decisions survive sessions."}]}"#);
        assert!(dream_if_due(&root, active, &model, &profile())
            .await
            .unwrap()
            .is_none());
        assert!(model.requests.lock().unwrap().is_empty());

        let mut fifth = crate::session::Session::fresh();
        fifth.messages.push(Message::text("user", "Keep durable decisions"));
        fifth.save(&root).unwrap();

        let result = dream_if_due(&root, active, &model, &profile())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.0, 1);
        assert!(root.join(".atlas/memory/consolidated.md").exists());
        assert!(root.join(".atlas/memory/.dream-consolidated").exists());
        let requests = model.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].tools.is_empty());
        assert!(requests[0].system.contains("Phase 3"));
        drop(requests);
        assert!(dream_if_due(&root, active, &model, &profile())
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn dream_error_does_not_advance_consolidation_timestamp() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path().canonicalize().unwrap();
        for _ in 0..5 {
            let mut session = crate::session::Session::fresh();
            session.messages.push(Message::text("user", "Important future decision"));
            session.save(&root).unwrap();
        }
        let active = uuid::Uuid::new_v4();
        let invalid = mock("This is not valid JSON");
        assert!(dream_if_due(&root, active, &invalid, &profile())
            .await
            .is_err());
        assert!(!root.join(".atlas/memory/.dream-consolidated").exists());
        let valid = mock(r#"{"memories":[]}"#);
        assert!(dream_if_due(&root, active, &valid, &profile())
            .await
            .unwrap()
            .is_some());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn rejects_symlinked_memory_directory() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join(".atlas")).unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join(".atlas/memory")).unwrap();
        let model = mock(r#"{"memories":[]}"#);
        assert!(extract(
            root.path(),
            &[Message::text("user", "hi")],
            0,
            "sys",
            &model,
            &profile()
        )
        .await
        .is_err());
    }
}
