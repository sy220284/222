use anyhow::{bail, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Message {
    pub from: String,
    pub text: String,
    pub timestamp: String,
    pub read: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}
#[derive(Clone)]
pub struct Mailbox {
    directory: PathBuf,
}
fn safe(input: &str) -> Result<String> {
    if input.is_empty() || input.len() > 120 {
        bail!("Invalid mailbox component")
    }
    Ok(input
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect())
}
impl Mailbox {
    pub fn new(root: &Path, team: &str) -> Result<Self> {
        let real_root = root.canonicalize()?;
        let base = real_root.join(".atlas/teams");
        fs::create_dir_all(&base)?;
        let base = base.canonicalize()?;
        if !base.starts_with(&real_root) {
            bail!("Team path escapes workspace")
        }
        let team_dir = base.join(safe(team)?);
        fs::create_dir_all(&team_dir)?;
        let real_team = team_dir.canonicalize()?;
        if !real_team.starts_with(&base) {
            bail!("Team directory escapes workspace")
        }
        let inbox_dir = real_team.join("inboxes");
        fs::create_dir_all(&inbox_dir)?;
        let inbox_dir = inbox_dir.canonicalize()?;
        if !inbox_dir.starts_with(&real_team) {
            bail!("Mailbox escapes team")
        }
        Ok(Self {
            directory: inbox_dir,
        })
    }
    fn path(&self, recipient: &str) -> Result<PathBuf> {
        let path = self.directory.join(format!("{}.json", safe(recipient)?));
        if path
            .symlink_metadata()
            .is_ok_and(|m| m.file_type().is_symlink())
        {
            bail!("Unsafe mailbox link")
        }
        Ok(path)
    }
    fn locked<T, F: FnOnce(&Path) -> Result<T>>(&self, recipient: &str, f: F) -> Result<T> {
        let path = self.path(recipient)?;
        let lock = path.with_extension("json.lock");
        if lock
            .symlink_metadata()
            .is_ok_and(|m| m.file_type().is_symlink())
        {
            bail!("Unsafe mailbox lock")
        }
        let mutex = OpenOptions::new().write(true).create(true).truncate(false).open(&lock)?;
        mutex.lock_exclusive()?;
        let result = f(&path);
        let _ = FileExt::unlock(&mutex);
        result
    }
    fn read_path(path: &Path) -> Result<Vec<Message>> {
        let data = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(err) => return Err(err.into()),
        };
        if data.len() > 8_000_000 {
            bail!("Mailbox too large")
        }
        Ok(serde_json::from_slice(&data).unwrap_or_default())
    }
    fn save(path: &Path, data: &[Message]) -> Result<()> {
        let encoded = serde_json::to_vec_pretty(data)?;
        if encoded.len() > 8_000_000 {
            bail!("Mailbox too large")
        }
        let tmp = path.with_extension(format!("{}.tmp", Uuid::new_v4()));
        let mut file = OpenOptions::new().create_new(true).write(true).open(&tmp)?;
        let outcome = (|| -> Result<()> {
            file.write_all(&encoded)?;
            file.sync_all()?;
            fs::rename(&tmp, path)?;
            Ok(())
        })();
        if outcome.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        outcome
    }
    pub fn read(&self, recipient: &str) -> Result<Vec<Message>> {
        Self::read_path(&self.path(recipient)?)
    }
    pub fn unread(&self, recipient: &str) -> Result<Vec<Message>> {
        Ok(self
            .read(recipient)?
            .into_iter()
            .filter(|item| !item.read)
            .collect())
    }
    pub fn write(&self, recipient: &str, mut message: Message) -> Result<()> {
        if message.text.len() > 200_000 {
            bail!("Message exceeds limit")
        }
        message.read = false;
        self.locked(recipient, |path| {
            let mut items = Self::read_path(path)?;
            items.push(message);
            Self::save(path, &items)
        })
    }
    pub fn mark_index(&self, recipient: &str, index: usize) -> Result<()> {
        self.locked(recipient, |path| {
            let mut items = Self::read_path(path)?;
            if let Some(item) = items.get_mut(index) {
                item.read = true;
                Self::save(path, &items)?;
            }
            Ok(())
        })
    }
    pub fn mark_all(&self, recipient: &str) -> Result<usize> {
        self.locked(recipient, |path| {
            let mut items = Self::read_path(path)?;
            let mut changed = 0usize;
            for item in &mut items {
                if !item.read {
                    item.read = true;
                    changed += 1;
                }
            }
            if changed > 0 {
                Self::save(path, &items)?;
            }
            Ok(changed)
        })
    }
    /// Acknowledge matching unread messages atomically without swallowing
    /// protocol messages intended for a separate dispatcher.
    pub fn take_matching<F>(&self, recipient: &str, predicate: F) -> Result<Vec<Message>>
    where
        F: Fn(&Message) -> bool,
    {
        self.locked(recipient, |path| {
            let mut items = Self::read_path(path)?;
            let mut taken = Vec::new();
            for item in &mut items {
                if !item.read && predicate(item) {
                    taken.push(item.clone());
                    item.read = true;
                }
            }
            if !taken.is_empty() {
                Self::save(path, &items)?;
            }
            Ok(taken)
        })
    }
    pub fn clear(&self, recipient: &str) -> Result<()> {
        self.locked(recipient, |path| Self::save(path, &[]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn message(label: String) -> Message {
        Message {
            from: "leader".into(),
            text: label,
            timestamp: "2026-10-28T10:00:00.000Z".into(),
            read: false,
            color: None,
            summary: None,
        }
    }
    #[test]
    fn writes_reads_marks_and_clears_durable_messages() {
        let tmp = tempfile::tempdir().unwrap();
        let mailbox = Mailbox::new(tmp.path(), "team").unwrap();
        mailbox.write("alice", message("one".into())).unwrap();
        mailbox.write("alice", message("two".into())).unwrap();
        mailbox.mark_index("alice", 0).unwrap();
        assert_eq!(mailbox.unread("alice").unwrap().len(), 1);
        let reloaded = Mailbox::new(tmp.path(), "team").unwrap();
        assert_eq!(reloaded.mark_all("alice").unwrap(), 1);
        assert_eq!(reloaded.read("alice").unwrap().len(), 2);
        assert!(reloaded.unread("alice").unwrap().is_empty());
        reloaded.clear("alice").unwrap();
        assert!(reloaded.read("alice").unwrap().is_empty());
    }
    #[test]
    fn concurrent_writes_do_not_lose_messages() {
        let root = std::sync::Arc::new(tempfile::tempdir().unwrap());
        let workers = (0..20)
            .map(|i| {
                let root = root.clone();
                std::thread::spawn(move || {
                    let m = Mailbox::new(root.path(), "team").unwrap();
                    m.write("alice", message(format!("message-{i}"))).unwrap();
                })
            })
            .collect::<Vec<_>>();
        for worker in workers {
            worker.join().unwrap();
        }
        let messages = Mailbox::new(root.path(), "team")
            .unwrap()
            .read("alice")
            .unwrap();
        assert_eq!(messages.len(), 20);
        assert_eq!(
            messages
                .iter()
                .map(|m| m.text.as_str())
                .collect::<std::collections::HashSet<_>>()
                .len(),
            20
        );
    }
    #[test]
    fn predicate_read_keeps_control_messages_unread() {
        let tmp = tempfile::tempdir().unwrap();
        let mailbox = Mailbox::new(tmp.path(), "team").unwrap();
        mailbox.write("alice", message("ordinary".into())).unwrap();
        mailbox
            .write("alice", message(r#"{"type":"permission_request"}"#.into()))
            .unwrap();
        let taken = mailbox
            .take_matching("alice", |m| !m.text.contains("permission_request"))
            .unwrap();
        assert_eq!(taken.len(), 1);
        assert_eq!(taken[0].text, "ordinary");
        assert_eq!(mailbox.unread("alice").unwrap().len(), 1);
        assert!(mailbox
            .take_matching("alice", |_| false)
            .unwrap()
            .is_empty());
        assert_eq!(mailbox.mark_all("alice").unwrap(), 1);
    }
    #[test]
    fn rejects_external_symlink_directory() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let root = tempfile::tempdir().unwrap();
            let other = tempfile::tempdir().unwrap();
            fs::create_dir_all(root.path().join(".atlas")).unwrap();
            symlink(other.path(), root.path().join(".atlas/teams")).unwrap();
            assert!(Mailbox::new(root.path(), "team").is_err());
        }
    }
}
