use crate::protocol::Message;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{path::{Path, PathBuf}, fs};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub messages: Vec<Message>,
}

impl Session {
    pub fn fresh() -> Self {
        Self { id: Uuid::new_v4(), messages: Vec::new() }
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
        let temp = path.with_extension("tmp");
        fs::write(&temp, data)?;
        fs::rename(temp, path)?;
        Ok(())
    }

    pub fn clear(&mut self) {
        self.messages.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
