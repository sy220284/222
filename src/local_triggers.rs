//! Locally persisted, explicitly invoked model-neutral task triggers.
//! No remote worker, background process, cloud schedule or implicit execution.
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use uuid::Uuid;

const MAX_TRIGGERS: usize = 128;
const MAX_PROMPT_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Trigger {
    pub prompt: String,
}

type Store = BTreeMap<String, Trigger>;

fn check_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > 64
        || !name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        bail!("Trigger name must be 1-64 ASCII letters, numbers, hyphens or underscores");
    }
    Ok(())
}

fn store_path(root: &Path) -> PathBuf {
    root.join(".atlas").join("local_triggers.json")
}

fn load(root: &Path) -> Result<Store> {
    let path = store_path(root);
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let meta = fs::metadata(&path).context("Cannot inspect trigger store")?;
    if meta.len() > (MAX_TRIGGERS * MAX_PROMPT_BYTES) as u64 {
        bail!("Local trigger store exceeds the accepted size limit");
    }
    let bytes = fs::read(&path).context("Cannot read trigger store")?;
    let entries: Store =
        serde_json::from_slice(&bytes).context("Local trigger store is corrupted")?;
    if entries.len() > MAX_TRIGGERS {
        bail!("Too many local triggers");
    }
    for (name, record) in &entries {
        check_name(name)?;
        if record.prompt.trim().is_empty() || record.prompt.len() > MAX_PROMPT_BYTES {
            bail!("Invalid stored trigger prompt for {name}");
        }
    }
    Ok(entries)
}

/// Use a create-new lock to reject concurrent writes rather than silently
/// losing an update; the lock's lifetime is exactly one write transaction.
struct Lock(PathBuf);
impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn update(root: &Path, change: impl FnOnce(&mut Store) -> Result<()>) -> Result<()> {
    let dir = root.join(".atlas");
    fs::create_dir_all(&dir).context("Cannot create local trigger directory")?;
    let lock_path = dir.join("local_triggers.lock");
    let mut options = fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let _lock_file = options
        .open(&lock_path)
        .context("Another trigger update is running")?;
    let _lock = Lock(lock_path);
    let mut entries = load(root)?;
    change(&mut entries)?;
    let path = store_path(root);
    let temp = dir.join(format!(".local_triggers-{}.tmp", Uuid::new_v4()));
    let result = (|| -> Result<()> {
        let bytes = serde_json::to_vec_pretty(&entries)?;
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temp)
            .context("Cannot create temporary trigger store")?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(&temp, &path).context("Cannot atomically replace trigger store")?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

pub fn set(root: &Path, name: &str, prompt: &str) -> Result<()> {
    check_name(name)?;
    let prompt = prompt.trim();
    if prompt.is_empty() || prompt.len() > MAX_PROMPT_BYTES {
        bail!("Trigger prompt must be 1-65536 bytes");
    }
    update(root, |entries| {
        if !entries.contains_key(name) && entries.len() >= MAX_TRIGGERS {
            bail!("Local trigger limit reached");
        }
        entries.insert(
            name.to_owned(),
            Trigger {
                prompt: prompt.to_owned(),
            },
        );
        Ok(())
    })
}

pub fn delete(root: &Path, name: &str) -> Result<()> {
    check_name(name)?;
    update(root, |entries| {
        if entries.remove(name).is_none() {
            bail!("No local trigger named {name}");
        }
        Ok(())
    })
}

pub fn get(root: &Path, name: &str) -> Result<String> {
    check_name(name)?;
    load(root)?
        .get(name)
        .map(|v| v.prompt.clone())
        .with_context(|| format!("No local trigger named {name}"))
}

pub fn list(root: &Path) -> Result<Vec<String>> {
    Ok(load(root)?.into_keys().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn triggers_persist_run_same_prompt_and_delete() {
        let root = tempfile::tempdir().unwrap();
        set(root.path(), "review_ci", "检查 CI 失败").unwrap();
        set(root.path(), "review_ci", "使用已配置模型检查 Git 变更").unwrap();
        assert_eq!(list(root.path()).unwrap(), vec!["review_ci"]);
        assert!(get(root.path(), "review_ci")
            .unwrap()
            .contains("已配置模型"));
        delete(root.path(), "review_ci").unwrap();
        assert!(get(root.path(), "review_ci").is_err());
        assert!(list(root.path()).unwrap().is_empty());
    }
    #[test]
    fn invalid_names_or_empty_tasks_do_not_write_store() {
        let root = tempfile::tempdir().unwrap();
        assert!(set(root.path(), "../outside", "message").is_err());
        assert!(set(root.path(), "valid", "  ").is_err());
        assert!(get(root.path(), "never").is_err());
        assert!(!store_path(root.path()).exists());
    }
    #[test]
    fn invalid_store_data_is_an_error_not_an_empty_success() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join(".atlas")).unwrap();
        fs::write(store_path(root.path()), b"invalid json").unwrap();
        assert!(list(root.path()).is_err());
        assert!(set(root.path(), "name", "prompt").is_err());
    }
}
