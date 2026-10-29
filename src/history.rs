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
    /// Hash of the original bytes, checked before presenting or restoring a backup.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_hash: Option<String>,
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
    // Validate the updated file metadata before creating any backup artifact.
    // A failed stat must not leave a dangling backup in session storage.
    let updated_mode = snapshot_mode(path)?;
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
        previous_hash: previous.as_ref().map(|bytes| fingerprint(bytes)),
        backup_path: backup_path.clone(),
        previous_mode,
        updated_hash: fingerprint(&now),
        updated_mode,
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
/// A failed pre-edit backup must not block the actual file edit. Keep the
/// failure distinct from an absent file: treating a failed read as None would
/// create a false "new file" checkpoint and could delete an existing file.
pub struct PendingCheckpoint {
    previous: Option<Vec<u8>>,
    previous_mode: Option<u32>,
    warning: Option<String>,
}

pub fn capture_before_edit(path: &Path) -> PendingCheckpoint {
    match snapshot(path).and_then(|previous| {
        snapshot_mode(path).map(|previous_mode| (previous, previous_mode))
    }) {
        Ok((previous, previous_mode)) => PendingCheckpoint {
            previous,
            previous_mode,
            warning: None,
        },
        Err(error) => PendingCheckpoint {
            previous: None,
            previous_mode: None,
            warning: Some(format!(
                "\nWarning: file edit may proceed, but its history backup could not be captured: {error:#}"
            )),
        },
    }
}

pub fn finish_after_edit(
    root: &Path,
    session_id: Option<&str>,
    path: &Path,
    pending: PendingCheckpoint,
) -> String {
    if let Some(warning) = pending.warning {
        return warning;
    }
    record_notice(root, session_id, path, pending.previous, pending.previous_mode)
}

/// FileHistoryTrackEdit logs a failed backup without converting an already
/// successful edit into a failed tool call. Surface the failure as a warning
/// so the user does not retry a write that has already reached disk.
pub fn record_notice(
    root: &Path,
    session_id: Option<&str>,
    path: &Path,
    previous: Option<Vec<u8>>,
    previous_mode: Option<u32>,
) -> String {
    match record(root, session_id, path, previous, previous_mode) {
        Ok(Some(id)) => format!("\nCheckpoint: {id}"),
        Ok(None) => String::new(),
        Err(error) => format!("\nWarning: file edit succeeded but checkpoint could not be saved: {error:#}"),
    }
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
/// Original fileHistory.ts::computeDiffStatsForFile compares the current
/// file against the saved version before the user chooses to rewind. A
/// checkpoint created for a new file has no previous content.
fn previous_bytes(record: &Record) -> Result<Option<Vec<u8>>> {
    let bytes = if record.previous_backup.is_some() {
        let backup = record.backup_path.as_ref()
            .context("History backup was not loaded from session storage")?;
        Some(fs::read(backup)
            .with_context(|| format!("Read history backup {}", backup.display()))?)
    } else {
        record.previous.as_ref().map(|text| text.as_bytes().to_vec())
    };
    if let (Some(expected), Some(bytes)) = (record.previous_hash.as_deref(), bytes.as_ref()) {
        if fingerprint(bytes) != expected {
            bail!("History backup contents changed; refusing to use a damaged checkpoint")
        }
    }
    Ok(bytes)
}

/// Read-only rewind preview, matching the source diff direction:
/// insertions are lines that would be restored, deletions are current lines
/// that would disappear. Binary files report a change without fake line counts.
pub fn preview(record: &Record, path: &Path) -> Result<String> {
    let previous = previous_bytes(record)?;
    let current = snapshot(path)?;
    let changed = current != previous;
    let restorable = match current.as_ref() {
        None => true,
        Some(bytes) => {
            fingerprint(bytes) == record.updated_hash
                && (record.updated_mode.is_none()
                    || snapshot_mode(path)? == record.updated_mode)
        }
    };
    let mut insertions = None;
    let mut deletions = None;
    let mut binary = false;
    if let (Some(current_text), Some(previous_text)) = (
        std::str::from_utf8(current.as_deref().unwrap_or_default()).ok(),
        std::str::from_utf8(previous.as_deref().unwrap_or_default()).ok(),
    ) {
        let diff = similar::TextDiff::from_lines(current_text, previous_text);
        let mut added = 0usize;
        let mut removed = 0usize;
        for change in diff.iter_all_changes() {
            match change.tag() {
                similar::ChangeTag::Insert => added += 1,
                similar::ChangeTag::Delete => removed += 1,
                similar::ChangeTag::Equal => {},
            }
        }
        insertions = Some(added);
        deletions = Some(removed);
    } else {
        binary = true;
    }
    Ok(serde_json::to_string_pretty(&json!({
        "id": record.id,
        "path": record.path,
        "changed": changed,
        "restorable": restorable,
        "insertions": insertions,
        "deletions": deletions,
        "binary": binary,
    }))?)
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
        // Verify the original bytes before any write can change the user's file.
        // This also detects a missing or corrupted backup before mutation.
        let _verified = previous_bytes(record)?;
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
    // fileHistory.ts keeps snapshots in insertion order. UUIDv4 lexical
    // ordering shuffled the visible checkpoint list on every session.
    let mut results = Vec::<(std::time::SystemTime, String, Value)>::new();
    for entry in fs::read_dir(folder)? {
        let entry = entry?;
        if entry.path().extension().and_then(|x| x.to_str()) != Some("json") {
            continue;
        }
        let path = entry.path();
        let data = fs::read(&path)
            .with_context(|| format!("Read file checkpoint {}", path.display()))?;
        let record: Record = serde_json::from_slice(&data)
            .with_context(|| format!("Parse file checkpoint {}", path.display()))?;
        if path.file_stem().and_then(|s| s.to_str()) != Some(record.id.to_string().as_str()) {
            bail!("History checkpoint filename/id mismatch: {}", path.display())
        }
        let modified = entry.metadata()?.modified()?;
        results.push((modified, record.id.to_string(),
            json!({"id":record.id,"path":record.path})));
    }
    results.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    Ok(serde_json::to_string_pretty(
        &results.into_iter().map(|(_, _, record)| record).collect::<Vec<_>>()
    )?)
}
pub fn definitions() -> Vec<Value> {
    vec![
        json!({"type":"function","function":{"name":"file_history",
            "description":"List local file checkpoints generated by file write, edit or notebook edits.",
            "parameters":{"type":"object","properties":{}}}}),
        json!({"type":"function","function":{"name":"file_history_diff",
            "description":"Preview line additions/deletions and whether a checkpoint can be safely restored without modifying files.",
            "parameters":{"type":"object","properties":{"id":{"type":"string"}},"required":["id"]}}}),
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
