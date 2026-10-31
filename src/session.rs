use crate::{
    cost::CostLedger,
    protocol::{Message, TokenUsage},
    task::Task,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub messages: Vec<Message>,
    /// Durable pre-compaction transcript. Never sent to the model again:
    /// the compact summary represents this portion in active context.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub archived_messages: Vec<Message>,
    /// Insertion-ordered user-turn IDs for message-level file snapshots.
    /// Old sessions simply have no rewindable user-turn index.
    #[serde(default)]
    pub history_message_ids: Vec<Uuid>,
    #[serde(default)]
    pub history_message_aliases: std::collections::BTreeMap<Uuid, Uuid>,
    #[serde(default)]
    pub summary: Option<String>,
    /// Model-visible summary position for source 2.1.88 prefix-preserving
    /// partial compaction. Full/up_to summaries appear at offset zero.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary_insert_at: Option<usize>,
    /// Source isCompactSummary state for earlier summaries retained by
    /// prefix-preserving compaction; indices refer to self.messages.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub retained_summary_indices: Vec<usize>,
    /// Immutable transcript prefix covering all turns before a partial
    /// compaction. Retained model messages may already occur in this prefix.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcript_snapshot: Option<Vec<Message>>,
    /// Number of active model turns already contained in transcript_snapshot.
    #[serde(default)]
    pub transcript_active_offset: usize,
    #[serde(default)]
    pub tasks: Vec<Task>,
    #[serde(default)]
    pub todos: Vec<crate::task::TodoItem>,
    /// Paths surfaced to the model during relevant-memory recall.
    #[serde(default)]
    pub surfaced_memories: Vec<String>,
    /// Cumulative UTF-16 units of memory attachments in this session.
    #[serde(default)]
    pub surfaced_memory_units: usize,
    /// Last message offset processed by optional local memory extraction.
    #[serde(default)]
    pub memory_extraction_cursor: usize,
    #[serde(default)]
    pub usage: TokenUsage,
    #[serde(default)]
    pub cost: CostLedger,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_color: Option<String>,
    /// SDK-generated title, separate from the user's explicit rename.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    /// Git branch captured when this conversation starts; never rewritten by later checkouts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forked_from: Option<Uuid>,
    /// Usage already charged to the parent, excluded from fork stats.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fork_baseline_usage: Option<TokenUsage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fork_baseline_cost: Option<CostLedger>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fork_baseline_messages: Option<usize>,
}

impl Session {
    pub fn fresh() -> Self {
        Self {
            id: Uuid::new_v4(),
            messages: Vec::new(),
            archived_messages: Vec::new(),
            history_message_ids: Vec::new(),
            history_message_aliases: std::collections::BTreeMap::new(),
            summary: None,
            summary_insert_at: None,
            retained_summary_indices: Vec::new(),
            transcript_snapshot: None,
            transcript_active_offset: 0,
            tasks: Vec::new(),
            todos: Vec::new(),
            surfaced_memories: Vec::new(),
            surfaced_memory_units: 0,
            memory_extraction_cursor: 0,
            usage: TokenUsage::default(),
            cost: CostLedger::default(),
            team_name: None,
            agent_name: None,
            permission_mode: None,
            custom_title: None,
            agent_color: None,
            ai_title: None,
            tag: None,
            git_branch: None,
            forked_from: None,
            fork_baseline_usage: None,
            fork_baseline_cost: None,
            fork_baseline_messages: None,
        }
    }

    fn path(root: &Path, id: Uuid) -> PathBuf {
        root.join(".atlas")
            .join("sessions")
            .join(format!("{id}.json"))
    }

    fn checked_path(root: &Path, id: Uuid) -> Result<PathBuf> {
        let root = root.canonicalize().context("Workspace unavailable")?;
        let base = root.join(".atlas/sessions");
        fs::create_dir_all(&base)?;
        let real = base.canonicalize()?;
        if !real.starts_with(&root) {
            anyhow::bail!("Session storage escapes workspace")
        }
        let path = real.join(format!("{id}.json"));
        if path
            .symlink_metadata()
            .is_ok_and(|m| m.file_type().is_symlink())
        {
            anyhow::bail!("Unsafe session file link")
        }
        Ok(path)
    }
    pub fn load(root: &Path, id: &str) -> Result<Self> {
        let id = Uuid::parse_str(id).context("Session id must be a UUID")?;
        let file = Self::checked_path(root, id)?;
        let data = fs::read(&file).with_context(|| format!("Cannot resume {}", file.display()))?;
        let session: Self = crate::json_runtime::parse_json(std::str::from_utf8(&data)?)?;
        if session.id != id {
            anyhow::bail!("Session id mismatch")
        }
        Ok(session)
    }

    pub fn save(&self, root: &Path) -> Result<()> {
        let path = Self::checked_path(root, self.id)?;
        fs::create_dir_all(path.parent().context("Invalid session path")?)?;
        let data = serde_json::to_vec_pretty(self)?;
        let temp = path.with_extension(format!("{}.tmp", Uuid::new_v4()));
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        let result = (|| -> Result<()> {
            file.write_all(&data)?;
            file.sync_all()?;
            fs::rename(&temp, &path)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result
    }

    /// Complete local transcript (archived and active) in chronological order.
    /// Keep this separate from model_messages: old turns must not be resent
    /// to the model after compaction has replaced them with a summary.
    pub fn transcript_messages(&self) -> Vec<Message> {
        let mut history = if let Some(snapshot) = &self.transcript_snapshot {
            snapshot.clone()
        } else {
            self.archived_messages.clone()
        };
        // Synthetic summaries are stored once at their compaction boundary.
        if let Some(summary) = self.summary.as_deref().filter(|s| !s.trim().is_empty()) {
            history.push(Message::text(
                "user",
                crate::compact_source_prompts::user_summary_message(summary, None, false, false),
            ));
        }
        let from = if self.transcript_snapshot.is_some() {
            self.transcript_active_offset.min(self.messages.len())
        } else {
            0
        };
        history.extend(self.messages[from..].iter().cloned());
        history
    }

    /// Source 2.1.88 places the compacted conversation summary back into
    /// the model transcript as a user message. Keep the persisted session
    /// normalized: the synthetic message is composed once per API request,
    /// never re-appended on retries, resume or per-tool model turns.
    pub fn model_messages(&self) -> Vec<Message> {
        let mut messages = self.messages.clone();
        if let Some(summary) = self.summary.as_deref().filter(|s| !s.trim().is_empty()) {
            let summary = Message::text(
                "user",
                crate::compact_source_prompts::user_summary_message(
                    summary,
                    (!self.archived_messages.is_empty() || self.transcript_snapshot.is_some())
                        .then(|| format!(".atlas/sessions/{}.json", self.id))
                        .as_deref(),
                    false,
                    false,
                ),
            );
            let insert_at = self.summary_insert_at.unwrap_or(0).min(messages.len());
            messages.insert(insert_at, summary);
        }
        messages
    }

    pub fn clear(&mut self) {
        self.summary = None;
        self.summary_insert_at = None;
        self.retained_summary_indices.clear();
        self.transcript_snapshot = None;
        self.transcript_active_offset = 0;
        self.messages.clear();
        self.archived_messages.clear();
        self.tasks.clear();
        self.surfaced_memories.clear();
        self.surfaced_memory_units = 0;
        self.memory_extraction_cursor = 0;
        self.todos.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compact_context_is_a_single_user_turn_and_not_a_system_override() {
        let mut session = Session::fresh();
        session.summary = Some("Summary:\nVerified state".to_owned());
        session
            .messages
            .push(Message::text("user", "Continue work"));
        let once = session.model_messages();
        let twice = session.model_messages();
        assert_eq!(once, twice);
        assert_eq!(session.messages.len(), 1);
        assert_eq!(once.len(), 2);
        assert_eq!(once[0].role, "user");
        assert!(once[0].visible_text().contains("Summary:\nVerified state"));
        assert_eq!(once[1].visible_text(), "Continue work");
        session.summary = None;
        assert_eq!(session.model_messages().len(), 1);
    }

    #[test]
    fn partial_compaction_preserves_model_position_without_duplicating_transcript() {
        let mut s = Session::fresh();
        s.messages = vec![
            Message::text("user", "kept-first"),
            Message::text("assistant", "kept-answer"),
        ];
        s.transcript_snapshot = Some(vec![
            Message::text("user", "kept-first"),
            Message::text("assistant", "kept-answer"),
            Message::text("user", "summarized-tail"),
        ]);
        s.transcript_active_offset = 2;
        s.summary = Some("Summary:\nSummarized tail".into());
        s.summary_insert_at = Some(2);
        s.retained_summary_indices.push(0);
        assert_eq!(s.model_messages().len(), 3);
        assert_eq!(s.model_messages()[0].visible_text(), "kept-first");
        assert_eq!(s.model_messages()[1].visible_text(), "kept-answer");
        assert!(s.model_messages()[2]
            .visible_text()
            .contains("Summarized tail"));
        s.messages.push(Message::text("user", "new-task"));
        let model = s.model_messages();
        assert_eq!(model[3].visible_text(), "new-task");
        let transcript = s.transcript_messages();
        assert_eq!(transcript.len(), 5);
        assert_eq!(transcript[2].visible_text(), "summarized-tail");
        assert_eq!(transcript[4].visible_text(), "new-task");
        s.summary_insert_at = Some(0);
        assert!(s.model_messages()[0]
            .visible_text()
            .contains("Summarized tail"));
        s.clear();
        assert!(s.transcript_snapshot.is_none());
        assert!(s.summary_insert_at.is_none());
        assert!(s.retained_summary_indices.is_empty());
    }

    #[test]
    fn session_storage_rejects_symlink_escape() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let root = tempfile::tempdir().unwrap();
            let external = tempfile::tempdir().unwrap();
            fs::create_dir_all(root.path().join(".atlas")).unwrap();
            symlink(external.path(), root.path().join(".atlas/sessions")).unwrap();
            let session = Session::fresh();
            assert!(session.save(root.path()).is_err());
            assert!(Session::load(root.path(), &session.id.to_string()).is_err());
        }
    }
    #[test]
    fn legacy_session_without_team_fields_remains_loadable() {
        let dir = tempfile::tempdir().unwrap();
        let s = Session::fresh();
        let path = Session::path(dir.path(), s.id);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            serde_json::json!({
                "id":s.id,"messages":[],"tasks":[],"usage":{
                    "input_tokens":0,"output_tokens":0,"total_tokens":0
                }
            })
            .to_string(),
        )
        .unwrap();
        let restored = Session::load(dir.path(), &s.id.to_string()).unwrap();
        assert!(restored.team_name.is_none());
        assert!(restored.agent_name.is_none());
        assert!(restored.permission_mode.is_none());
    }
    #[test]
    fn session_resume_accepts_bom_from_windows_tools() {
        let root = tempfile::tempdir().unwrap();
        let session = Session::fresh();
        let path = Session::path(root.path(), session.id);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let raw = serde_json::to_string(&session).unwrap();
        fs::write(&path, format!("{}{}", '\u{feff}', raw)).unwrap();
        let restored = Session::load(root.path(), &session.id.to_string()).unwrap();
        assert_eq!(restored.id, session.id);
        assert!(restored.cost.models.is_empty());
    }
    #[test]
    fn roundtrip_resume() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Session::fresh();
        s.messages.push(Message::text("user", "hello"));
        s.permission_mode = Some("acceptEdits".into());
        s.save(dir.path()).unwrap();
        let loaded = Session::load(dir.path(), &s.id.to_string()).unwrap();
        assert_eq!(loaded.messages, s.messages);
        assert_eq!(loaded.permission_mode.as_deref(), Some("acceptEdits"));
        assert!(Session::load(dir.path(), "../invalid").is_err());
    }
}
