use crate::{protocol::{Message, TokenUsage}, task::Task};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{path::{Path, PathBuf}, fs};
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
    pub usage: TokenUsage,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub team_name: Option<String>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub agent_name: Option<String>,
}

impl Session {
    pub fn fresh() -> Self {
        Self { id: Uuid::new_v4(), messages: Vec::new(), summary: None, tasks: Vec::new(), usage: TokenUsage::default(), team_name: None, agent_name: None }
    }

    fn path(root: &Path, id: Uuid) -> PathBuf {
        root.join(".atlas").join("sessions").join(format!("{id}.json"))
    }

    pub fn load(root: &Path, id: &str) -> Result<Self> {
        let id = Uuid::parse_str(id).context("Session id must be a UUID")?;
        let file = Self::path(root, id);
        let data = fs::read(&file)
            .with_context(|| format!("Cannot resume {}", file.display()))?;
        let session: Self = serde_json::from_slice(&data)?;
        if session.id != id { anyhow::bail!("Session id mismatch") }
        Ok(session)
    }

    pub fn save(&self, root: &Path) -> Result<()> {
        let path = Self::path(root, self.id);
        fs::create_dir_all(path.parent().context("Invalid session path")?)?;
        let data = serde_json::to_vec_pretty(self)?;
        let temp=path.with_extension(format!("{}.tmp",Uuid::new_v4()));
        use std::io::Write;
        let mut file=fs::OpenOptions::new().write(true).create_new(true).open(&temp)?;
        let result=(||->Result<()>{
            file.write_all(&data)?;
            file.sync_all()?;
            fs::rename(&temp,&path)?;
            Ok(())
        })();
        if result.is_err(){let _=fs::remove_file(&temp);}
        result
    }

    pub fn clear(&mut self) {
        self.summary = None;
        self.messages.clear();
        self.tasks.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_session_without_team_fields_remains_loadable(){
        let dir=tempfile::tempdir().unwrap();
        let s=Session::fresh();
        let path=Session::path(dir.path(),s.id);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path,serde_json::json!({
            "id":s.id,"messages":[],"tasks":[],"usage":{
                "input_tokens":0,"output_tokens":0,"total_tokens":0
            }
        }).to_string()).unwrap();
        let restored=Session::load(dir.path(),&s.id.to_string()).unwrap();
        assert!(restored.team_name.is_none());
        assert!(restored.agent_name.is_none());
    }
    #[test]
    fn roundtrip_resume() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Session::fresh();
        s.messages.push(Message::text("user", "hello"));
        s.save(dir.path()).unwrap();
        let loaded = Session::load(dir.path(), &s.id.to_string()).unwrap();
        assert_eq!(loaded.messages, s.messages);
        assert!(Session::load(dir.path(), "../invalid").is_err());
    }
}
