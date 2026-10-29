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
    #[serde(default)]
    pub summary: Option<String>,
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
    pub tag: Option<String>,
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
            summary: None,
            tasks: Vec::new(),
            todos: Vec::new(),
            surfaced_memories: Vec::new(),
            surfaced_memory_units: 0,
            usage: TokenUsage::default(),
            cost: CostLedger::default(),
            team_name: None,
            agent_name: None,
            permission_mode: None,
            custom_title: None,
            tag: None,
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
        let session: Self =
            serde_json::from_str(crate::json_read::strip_bom(std::str::from_utf8(&data)?))?;
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

    pub fn clear(&mut self) {
        self.summary = None;
        self.messages.clear();
        self.tasks.clear();
        self.surfaced_memories.clear();
        self.surfaced_memory_units = 0;
        self.todos.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
