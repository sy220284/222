use anyhow::{bail, Context, Result};
use std::io::Write as _;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};
use uuid::Uuid;

/// Original fileHistoryEnabled: interactive sessions honor the user setting;
/// headless/SDK sessions require explicit opt-in. Disable always wins.
fn env_truthy(name: &str) -> bool {
    std::env::var(name).ok().is_some_and(|value| {
        matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on")
    })
}
pub fn enabled(interactive: bool, user_enabled: bool) -> bool {
    if env_truthy("ATLAS_DISABLE_FILE_CHECKPOINTING") {
        return false;
    }
    if interactive {
        user_enabled
    } else {
        env_truthy("ATLAS_ENABLE_SDK_FILE_CHECKPOINTING")
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Record {
    pub id: Uuid,
    pub path: String,
    /// Inline text is retained for checkpoints created by earlier versions.
    pub previous: Option<String>,
    /// New checkpoints keep a byte-for-byte backup on disk. This preserves
    /// non-UTF-8 content and avoids a hard file-size cap or JSON/base64 bloat.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_backup: Option<String>,
    #[serde(skip)]
    backup_path: Option<PathBuf>,
    /// File mode before the edit, preserved by the original file-history backup.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_mode: Option<u32>,
    pub updated_hash: String,
    /// Mode immediately after the edit; changes since the checkpoint invalidate rewind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_mode: Option<u32>,
}
fn fingerprint(content: &[u8]) -> String {
    format!("{:x}", Sha256::digest(content))
}
fn storage(root: &Path, session_id: Option<&str>) -> Result<PathBuf> {
    let root = root.canonicalize().context("History workspace unavailable")?;
    let base = root.join(".atlas/history");
    fs::create_dir_all(&base)?;
    let base = base.canonicalize()?;
    if !base.starts_with(&root) {
        bail!("History directory escapes workspace")
    }
    // Original fileHistory.ts::resolveBackupPath isolates backup data by session.
    let Some(session_id) = session_id else { return Ok(base) };
    let session = Uuid::parse_str(session_id).context("History session id must be a UUID")?;
    let folder = base.join(session.to_string());
    fs::create_dir_all(&folder)?;
    if folder.symlink_metadata()?.file_type().is_symlink() {
        bail!("History session directory must not be a symbolic link")
    }
    let folder = folder.canonicalize()?;
    if !folder.starts_with(&base) {
        bail!("History session directory escapes workspace")
    }
    Ok(folder)
}
pub fn snapshot(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}
/// Capture the original Unix file permissions before a Write/Edit/NotebookEdit.
pub fn snapshot_mode(path: &Path) -> Result<Option<u32>> {
    match fs::metadata(path) {
        Ok(metadata) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                Ok(Some(metadata.permissions().mode()))
            }
            #[cfg(not(unix))]
            {
                let _ = metadata;
                Ok(None)
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub fn record(
    root: &Path,
    session_id: Option<&str>,
    path: &Path,
    previous: Option<Vec<u8>>,
    previous_mode: Option<u32>,
) -> Result<Option<Uuid>> {
    let now = fs::read(path)?;
    if previous.as_ref().is_some_and(|p| p.as_slice() == now.as_slice()) {
        return Ok(None);
    }
    let relative = path.strip_prefix(root).unwrap_or(path);
    let id = Uuid::new_v4();
    let folder = storage(root, session_id)?;
    let backup_name = previous.as_ref().map(|_| format!("{id}.bak"));
    let backup_path = backup_name.as_ref().map(|name| folder.join(name));
    // A separate binary backup mirrors fileHistory.ts::createBackup: binary
    // notebooks and large files never have to pass through JSON text fields.
    if let (Some(contents), Some(backup)) = (previous.as_ref(), backup_path.as_ref()) {
        let mut file = fs::OpenOptions::new()
            .write(true).create_new(true).open(backup)
            .with_context(|| format!("Create history backup {}", backup.display()))?;
        if let Err(error) = file.write_all(contents).and_then(|_| file.sync_all()) {
            drop(file);
            let _ = fs::remove_file(backup);
            return Err(error.into());
        }
    }
    let entry = Record {
        id,
        path: relative.to_string_lossy().to_string(),
        previous: None,
        previous_backup: backup_name,
        backup_path: backup_path.clone(),
        previous_mode,
        updated_hash: fingerprint(&now),
        updated_mode: snapshot_mode(path)?,
    };
    let final_path = folder.join(format!("{id}.json"));
    let temporary = folder.join(format!("{id}.tmp"));
    let outcome = (|| -> Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true).create_new(true).open(&temporary)?;
        file.write_all(&serde_json::to_vec_pretty(&entry)?)?;
        file.sync_all()?;
        fs::rename(&temporary, &final_path)?;
        Ok(())
    })();
    if outcome.is_err() {
        let _ = fs::remove_file(&temporary);
        if let Some(backup) = backup_path {
            let _ = fs::remove_file(backup);
        }
    }
    outcome?;
    Ok(Some(id))
}
pub fn load(root: &Path, session_id: Option<&str>, id: &str) -> Result<Record> {
    let id = Uuid::parse_str(id).context("History id must be a UUID")?;
    let folder = storage(root, session_id)?;
    let path = folder.join(format!("{id}.json"));
    let mut record: Record = serde_json::from_slice(&fs::read(path)?)?;
    if record.id != id {
        bail!("History id mismatch")
    }
    if let Some(name) = record.previous_backup.as_deref() {
        if name != format!("{id}.bak") {
            bail!("History backup name does not match checkpoint")
        }
        let backup = folder.join(name);
        if fs::symlink_metadata(&backup)?.file_type().is_symlink() {
            bail!("History backup must not be a symbolic link")
        }
        record.backup_path = Some(backup);
    }
    Ok(record)
}
pub fn restore(record: &Record, path: &Path) -> Result<String> {
    // Original applySnapshot/restoreBackup can restore a tracked file that was
    // deleted after the snapshot, and recreates missing parent directories.
    let current = match fs::read(path) {
        Ok(content) => Some(content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    if let Some(current) = &current {
        if fingerprint(current) != record.updated_hash {
            bail!("File changed since checkpoint; refusing to overwrite later edits")
        }
        if let Some(expected_mode) = record.updated_mode {
            if snapshot_mode(path)? != Some(expected_mode) {
                bail!("File permissions changed since checkpoint; refusing to overwrite later edits")
            }
        }
    }
    if record.previous_backup.is_some() || record.previous.is_some() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        if record.previous_backup.is_some() {
            let backup = record.backup_path.as_ref()
                .context("History backup was not loaded from session storage")?;
            fs::copy(backup, path)
                .with_context(|| format!("Restore history backup {}", backup.display()))?;
        } else if let Some(previous) = &record.previous {
            fs::write(path, previous)?;
        }
        #[cfg(unix)]
        if let Some(mode) = record.previous_mode {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
        }
        Ok(format!("Restored {} to previous version", record.path))
    } else {
        if current.is_some() {
            fs::remove_file(path)?;
        }
        Ok(format!("Removed newly created {} (if present)", record.path))
    }
}
pub fn list(root: &Path, session_id: Option<&str>) -> Result<String> {
    let folder = storage(root, session_id)?;
    let mut results = Vec::<Value>::new();
    for entry in fs::read_dir(folder)? {
        let entry = entry?;
        if entry.path().extension().and_then(|x| x.to_str()) != Some("json") {
            continue;
        }
        if let Ok(data) = fs::read(entry.path()) {
            if let Ok(record) = serde_json::from_slice::<Record>(&data) {
                results.push(json!({"id":record.id,"path":record.path}));
            }
        }
    }
    results.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    Ok(serde_json::to_string_pretty(&results)?)
}
pub fn definitions() -> Vec<Value> {
    vec![
        json!({"type":"function","function":{"name":"file_history",
            "description":"List local file checkpoints generated by file write, edit or notebook edits.",
            "parameters":{"type":"object","properties":{}}}}),
        json!({"type":"function","function":{"name":"file_revert",
            "description":"Restore a checkpoint only if the current file still matches the captured edited version. Requires approval.",
            "parameters":{"type":"object","properties":{"id":{"type":"string"}},"required":["id"]}}}),
    ]
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checkpoints_restore_changes_and_refuse_stale_revert() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("file.txt");
        fs::write(&source, "old").unwrap();
        let before = snapshot(&source).unwrap();
        fs::write(&source, "new").unwrap();
        let id = record(root.path(), None, &source, before, None).unwrap().unwrap();
        assert!(list(root.path(), None).unwrap().contains(&id.to_string()));
        let entry = load(root.path(), None, &id.to_string()).unwrap();
        fs::write(&source, "external change").unwrap();
        assert!(restore(&entry, &source).is_err());
        fs::write(&source, "new").unwrap();
        restore(&entry, &source).unwrap();
        assert_eq!(fs::read_to_string(&source).unwrap(), "old");

        let created = root.path().join("created.txt");
        let before = snapshot(&created).unwrap();
        assert!(before.is_none());
        fs::write(&created, "hello").unwrap();
        let id = record(root.path(), None, &created, before, None).unwrap().unwrap();
        restore(&load(root.path(), None, &id.to_string()).unwrap(), &created).unwrap();
        assert!(!created.exists());
    }
}
