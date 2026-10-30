use anyhow::{bail, Context, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::Write as _;
use std::{
    fs,
    path::{Path, PathBuf},
};
use uuid::Uuid;

/// Original fileHistoryEnabled: interactive sessions honor the user setting;
/// headless/SDK sessions require explicit opt-in. Disable always wins.
fn env_truthy(name: &str) -> bool {
    std::env::var(name).ok().is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_id: Option<Uuid>,
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

/// Read only a regular checkpoint file, including across the open itself.
/// The no-follow flag closes the gap between symlink_metadata and fs::read.
fn open_history_file(path: &Path) -> Result<fs::File> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() {
        bail!(
            "History checkpoint must be a regular file: {}",
            path.display()
        );
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        bail!(
            "History checkpoint changed to a non-file: {}",
            path.display()
        );
    }
    Ok(file)
}

fn read_checkpoint_file(path: &Path) -> Result<Vec<u8>> {
    let mut file = open_history_file(path)?;
    let mut bytes = Vec::new();
    use std::io::Read as _;
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}
/// Keep checkpoint backups under the user's private configuration directory,
/// as in fileHistory.ts::resolveBackupPath. Workspace-local backups can be
/// accidentally indexed or committed alongside project source files.
fn private_history_dir(path: &Path) -> Result<PathBuf> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            bail!(
                "File history directory is redirected or invalid: {}",
                path.display()
            )
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(path).with_context(|| format!("Create {}", path.display()))?;
        }
        Err(error) => return Err(error.into()),
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    path.canonicalize()
        .context("File history directory unavailable")
}

/// Move existing workspace-local checkpoints into private storage without
/// dropping any backup. The staged copy is published only after all files
/// have been copied and synced; the old directory survives failed migration.
fn migrate_workspace_history(source: &Path, target: &Path, direct_files_only: bool) -> Result<()> {
    let source_metadata = match fs::symlink_metadata(source) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if source_metadata.file_type().is_symlink() || !source_metadata.is_dir() {
        bail!("Previous file history directory is redirected or invalid")
    }
    let mut files = Vec::new();
    for item in fs::read_dir(source)? {
        let item = item?;
        let metadata = fs::symlink_metadata(item.path())?;
        if direct_files_only && metadata.is_dir() && !metadata.file_type().is_symlink() {
            continue;
        }
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            bail!(
                "Previous file history contains a redirected or invalid entry: {}",
                item.path().display()
            )
        }
        files.push(item);
    }
    if files.is_empty() {
        return Ok(());
    }
    if target.exists() && fs::read_dir(target)?.next().is_some() {
        // A previous migration may have published the private copy before
        // being interrupted while deleting old files. Only remove old files
        // when every remaining byte matches the private checkpoint exactly.
        for item in &files {
            let private = target.join(item.file_name());
            let old = fs::read(item.path())?;
            let copied = fs::read(&private)
                .with_context(|| format!("History migration conflict: {}", private.display()))?;
            if old != copied {
                bail!("History migration conflict: workspace and private checkpoints differ")
            }
        }
        for item in &files {
            fs::remove_file(item.path())?;
        }
        if !direct_files_only {
            fs::remove_dir(source)?;
        }
        return Ok(());
    }
    let parent = target
        .parent()
        .context("History destination has no parent")?;
    let staged = parent.join(format!(".migrate-{}", Uuid::new_v4()));
    private_history_dir(&staged)?;
    let outcome = (|| -> Result<()> {
        for item in &files {
            let from = item.path();
            let to = staged.join(item.file_name());
            let mut input = fs::File::open(&from)?;
            let mut output = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&to)?;
            std::io::copy(&mut input, &mut output)?;
            output.sync_all()?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&to, fs::Permissions::from_mode(0o600))?;
            }
        }
        if target.exists() {
            fs::remove_dir(target)?;
        }
        fs::rename(&staged, target)?;
        Ok(())
    })();
    if let Err(error) = outcome {
        if let Err(cleanup) = fs::remove_dir_all(&staged) {
            bail!("History migration failed: {error:#}; staged cleanup failed: {cleanup:#}")
        }
        return Err(error);
    }
    for item in &files {
        fs::remove_file(item.path()).with_context(|| {
            format!(
                "Private history is saved but the old checkpoint could not be removed: {}",
                item.path().display()
            )
        })?;
    }
    if !direct_files_only {
        fs::remove_dir(source)?;
    }
    Ok(())
}
fn storage(root: &Path, session_id: Option<&str>) -> Result<PathBuf> {
    let root = root
        .canonicalize()
        .context("History workspace unavailable")?;
    let config_path = crate::config::Config::path()?;
    let config_parent = config_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(config_parent)?;
    let config_parent = config_parent.canonicalize()?;
    let base = private_history_dir(&config_parent.join("file-history"))?;
    let session = match session_id {
        Some(id) => Uuid::parse_str(id)
            .context("History session id must be a UUID")?
            .to_string(),
        None => format!(
            "workspace-{:x}",
            Sha256::digest(root.to_string_lossy().as_bytes())
        ),
    };
    let folder = base.join(&session);
    let legacy_base = root.join(".atlas/history");
    let legacy = if session_id.is_some() {
        legacy_base.join(&session)
    } else {
        legacy_base.clone()
    };
    // An existing .atlas symlink must not be used as a migration source.
    for ancestor in [root.join(".atlas"), legacy_base.clone()] {
        match fs::symlink_metadata(&ancestor) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                bail!(
                    "Previous history storage is redirected: {}",
                    ancestor.display()
                )
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    if legacy_base.exists() {
        let real = legacy_base.canonicalize()?;
        if !real.starts_with(&root) {
            bail!("Previous history storage escapes workspace")
        }
    }
    migrate_workspace_history(&legacy, &folder, session_id.is_none())?;
    private_history_dir(&folder)
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

const MAX_HISTORY_SNAPSHOTS: usize = 100;
fn lock_history(folder: &Path) -> Result<fs::File> {
    let path = folder.join(".checkpoint.lock");
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let file = options
        .open(&path)
        .with_context(|| format!("Open checkpoint lock {}", path.display()))?;
    if !file.metadata()?.is_file() {
        bail!("Checkpoint lock is not a regular file");
    }
    file.lock_exclusive().context("Lock session checkpoints")?;
    Ok(file)
}
/// Match the source's 100 most recent message snapshots, counting all edits
/// of one turn together. Old Rust checkpoints without a turn ID stand alone.
fn prune_history(folder: &Path) -> Result<()> {
    use std::collections::HashMap;
    use std::time::SystemTime;
    let mut groups: HashMap<Uuid, (SystemTime, Vec<(PathBuf, Option<PathBuf>)>)> = HashMap::new();
    for entry in fs::read_dir(folder)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|part| part.to_str()) != Some("json") {
            continue;
        }
        let name = path
            .file_stem()
            .and_then(|part| part.to_str())
            .context("Invalid checkpoint filename")?;
        let id = Uuid::parse_str(name).context("Checkpoint filename must be a UUID")?;
        let record: Record = serde_json::from_slice(&read_checkpoint_file(&path)?)
            .with_context(|| format!("Read checkpoint {}", path.display()))?;
        if record.id != id {
            bail!("Checkpoint filename/id mismatch");
        }
        let backup = match record.previous_backup {
            Some(name) if name == format!("{id}.bak") => Some(folder.join(name)),
            Some(_) => bail!("Invalid checkpoint backup name"),
            None => None,
        };
        let modified = entry.metadata()?.modified()?;
        let group = groups
            .entry(record.message_id.unwrap_or(id))
            .or_insert_with(|| (modified, Vec::new()));
        if modified < group.0 {
            group.0 = modified;
        }
        group.1.push((path, backup));
    }
    if groups.len() <= MAX_HISTORY_SNAPSHOTS {
        return Ok(());
    }
    let mut ordered = groups.into_iter().collect::<Vec<_>>();
    ordered.sort_by(|a, b| a.1 .0.cmp(&b.1 .0).then_with(|| a.0.cmp(&b.0)));
    let count = ordered.len() - MAX_HISTORY_SNAPSHOTS;
    for (_, (_, records)) in ordered.into_iter().take(count) {
        for (checkpoint, backup) in records {
            // Remove the index first, so an interrupted cleanup can leave
            // an orphan backup but never an index pointing at missing bytes.
            fs::remove_file(&checkpoint)?;
            if let Some(backup) = backup {
                match fs::remove_file(&backup) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => {
                        return Err(error)
                            .with_context(|| format!("Remove old backup {}", backup.display()))
                    }
                }
            }
        }
    }
    Ok(())
}

pub fn record(
    root: &Path,
    session_id: Option<&str>,
    path: &Path,
    previous: Option<Vec<u8>>,
    previous_mode: Option<u32>,
) -> Result<Option<Uuid>> {
    record_for_message(root, session_id, path, previous, previous_mode, None)
}
pub fn record_for_message(
    root: &Path,
    session_id: Option<&str>,
    path: &Path,
    previous: Option<Vec<u8>>,
    previous_mode: Option<u32>,
    message_id: Option<Uuid>,
) -> Result<Option<Uuid>> {
    let now = fs::read(path)?;
    if previous
        .as_ref()
        .is_some_and(|p| p.as_slice() == now.as_slice())
    {
        return Ok(None);
    }
    // Validate the updated file metadata before creating any backup artifact.
    // A failed stat must not leave a dangling backup in session storage.
    let updated_mode = snapshot_mode(path)?;
    let relative = path.strip_prefix(root).unwrap_or(path);
    let id = Uuid::new_v4();
    let folder = storage(root, session_id)?;
    let _lock = lock_history(&folder)?;
    // Keep the pre-turn backup even when several Edit/Write calls touch
    // the same file; only the expected post-edit hash advances.
    if let Some(turn) = message_id {
        for entry in fs::read_dir(&folder)? {
            let entry = entry?;
            let candidate = entry.path();
            if candidate.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let mut old: Record = serde_json::from_slice(&read_checkpoint_file(&candidate)?)
                .with_context(|| format!("Read prior turn checkpoint {}", candidate.display()))?;
            if old.message_id != Some(turn) || old.path != relative.to_string_lossy() {
                continue;
            }
            if candidate.file_stem().and_then(|s| s.to_str()) != Some(old.id.to_string().as_str()) {
                bail!("Prior turn checkpoint filename/id mismatch");
            }
            old.updated_hash = fingerprint(&now);
            old.updated_mode = updated_mode;
            let staged = folder.join(format!(".{}.update-{}.tmp", old.id, Uuid::new_v4()));
            let update = (|| -> Result<()> {
                let mut file = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&staged)?;
                file.write_all(&serde_json::to_vec_pretty(&old)?)?;
                file.sync_all()?;
                fs::rename(&staged, &candidate)?;
                Ok(())
            })();
            if update.is_err() {
                let _ = fs::remove_file(&staged);
            }
            update?;
            return Ok(Some(old.id));
        }
    }
    let backup_name = previous.as_ref().map(|_| format!("{id}.bak"));
    let backup_path = backup_name.as_ref().map(|name| folder.join(name));
    // A separate binary backup mirrors fileHistory.ts::createBackup: binary
    // notebooks and large files never have to pass through JSON text fields.
    if let (Some(contents), Some(backup)) = (previous.as_ref(), backup_path.as_ref()) {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(backup)
            .with_context(|| format!("Create history backup {}", backup.display()))?;
        if let Err(error) = file.write_all(contents).and_then(|_| file.sync_all()) {
            drop(file);
            let _ = fs::remove_file(backup);
            return Err(error.into());
        }
    }
    let entry = Record {
        id,
        message_id,
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
            .write(true)
            .create_new(true)
            .open(&temporary)?;
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
    prune_history(&folder)?;
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
    message_id: Option<Uuid>,
) -> String {
    if let Some(warning) = pending.warning {
        return warning;
    }
    record_notice_for_message(
        root,
        session_id,
        path,
        pending.previous,
        pending.previous_mode,
        message_id,
    )
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
    record_notice_for_message(root, session_id, path, previous, previous_mode, None)
}
pub fn record_notice_for_message(
    root: &Path,
    session_id: Option<&str>,
    path: &Path,
    previous: Option<Vec<u8>>,
    previous_mode: Option<u32>,
    message_id: Option<Uuid>,
) -> String {
    match record_for_message(root, session_id, path, previous, previous_mode, message_id) {
        Ok(Some(id)) => format!("\nCheckpoint: {id}"),
        Ok(None) => String::new(),
        Err(error) => {
            format!("\nWarning: file edit succeeded but checkpoint could not be saved: {error:#}")
        }
    }
}

pub fn load(root: &Path, session_id: Option<&str>, id: &str) -> Result<Record> {
    let id = Uuid::parse_str(id).context("History id must be a UUID")?;
    let folder = storage(root, session_id)?;
    let path = folder.join(format!("{id}.json"));
    let mut record: Record = serde_json::from_slice(&read_checkpoint_file(&path)?)?;
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
        let backup = record
            .backup_path
            .as_ref()
            .context("History backup was not loaded from session storage")?;
        Some(
            read_checkpoint_file(backup)
                .with_context(|| format!("Read history backup {}", backup.display()))?,
        )
    } else {
        record
            .previous
            .as_ref()
            .map(|text| text.as_bytes().to_vec())
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
                && (record.updated_mode.is_none() || snapshot_mode(path)? == record.updated_mode)
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
                similar::ChangeTag::Equal => {}
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

/// Restore verified checkpoint bytes via a same-directory replacement. A failed
/// write must leave the current user file intact, including on disk-full errors.
fn publish_verified_restore(path: &Path, bytes: &[u8], mode: Option<u32>) -> Result<()> {
    // Preserve the original copyFile behavior for a user file that is a
    // symbolic link: replace its resolved target, keeping the link in place.
    // A dangling link is rejected rather than redirecting the staged write.
    let destination = match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => path
            .canonicalize()
            .with_context(|| format!("Resolve history restore target {}", path.display()))?,
        Ok(_) => path.to_path_buf(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => path.to_path_buf(),
        Err(error) => return Err(error.into()),
    };
    let parent = destination
        .parent()
        .filter(|directory| !directory.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let name = destination
        .file_name()
        .context("History restore target has no filename")?;
    let temporary = parent.join(format!(
        ".{}.history-{}.tmp",
        name.to_string_lossy(),
        Uuid::new_v4()
    ));
    let result = (|| -> Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .with_context(|| format!("Create staged history restore {}", temporary.display()))?;
        file.write_all(bytes)?;
        #[cfg(unix)]
        if let Some(mode) = mode {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(fs::Permissions::from_mode(mode))?;
        }
        #[cfg(not(unix))]
        let _ = mode;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, &destination)
            .with_context(|| format!("Publish history restore {}", destination.display()))?;
        Ok(())
    })();
    if let Err(error) = result {
        match fs::remove_file(&temporary) {
            Ok(()) => Err(error),
            Err(cleanup) if cleanup.kind() == std::io::ErrorKind::NotFound => Err(error),
            Err(cleanup) => Err(error.context(format!(
                "Failed to clean staged history restore {}: {cleanup}",
                temporary.display()
            ))),
        }
    } else {
        Ok(())
    }
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
                bail!(
                    "File permissions changed since checkpoint; refusing to overwrite later edits"
                )
            }
        }
    }
    if record.previous_backup.is_some() || record.previous.is_some() {
        // Use the exact bytes already verified by SHA-256. Reopening the
        // backup with fs::copy after verification allowed a replaced backup
        // path to write different bytes during the rewind.
        let verified =
            previous_bytes(record)?.context("History checkpoint has no previous content")?;
        publish_verified_restore(path, &verified, record.previous_mode)?;
        Ok(format!("Restored {} to previous version", record.path))
    } else {
        if current.is_some() {
            fs::remove_file(path)?;
        }
        Ok(format!(
            "Removed newly created {} (if present)",
            record.path
        ))
    }
}
/// Resolve saved file edits for a user-message suffix in reverse turn order.
/// Each checkpoint retains its verified pre-edit bytes; the source rewind
/// replays the tracked-files state across all messages after the target.
pub fn records_for_message_sequence(
    root: &Path,
    session_id: &str,
    message_ids: &[Uuid],
) -> Result<Vec<Record>> {
    use std::collections::HashMap;
    if message_ids.is_empty() {
        return Ok(Vec::new());
    }
    let folder = storage(root, Some(session_id))?;
    let order = message_ids
        .iter()
        .enumerate()
        .map(|(position, id)| (*id, position))
        .collect::<HashMap<Uuid, usize>>();
    let mut found = Vec::new();
    for item in fs::read_dir(&folder)? {
        let item = item?;
        let path = item.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.file_type().is_file() {
            bail!("Rewind checkpoint must be a regular file");
        }
        let id = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .context("Rewind checkpoint filename is not valid UTF-8")?;
        let record = load(root, Some(session_id), id)?;
        if let Some(position) = record.message_id.and_then(|id| order.get(&id).copied()) {
            found.push((position, metadata.modified()?, record));
        }
    }
    // Descending turn order, newest checkpoint first; sorting by modification
    // time within a turn avoids overwriting newer state from a parallel edit.
    found.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| b.1.cmp(&a.1))
            .then_with(|| b.2.id.cmp(&a.2.id))
    });
    Ok(found.into_iter().map(|(_, _, record)| record).collect())
}

/// Simulate the entire multi-file rewind before the first disk mutation.
/// Conflicts, corrupt binary backups, and out-of-order writes all fail early;
/// file edits by another process must never be silently overwritten.
pub fn preflight_message_rewind(records: &[(Record, PathBuf)]) -> Result<()> {
    use std::collections::HashMap;
    let mut simulated = HashMap::<PathBuf, (Option<Vec<u8>>, Option<u32>)>::new();
    for (record, path) in records {
        let (current, mode) = if let Some(previous) = simulated.get(path) {
            previous.clone()
        } else {
            (snapshot(path)?, snapshot_mode(path)?)
        };
        if let Some(content) = current.as_ref() {
            if fingerprint(content) != record.updated_hash {
                bail!(
                    "File changed after checkpoint; refusing message rewind: {}",
                    path.display()
                );
            }
            if record.updated_mode.is_some() && mode != record.updated_mode {
                bail!(
                    "File permissions changed after checkpoint: {}",
                    path.display()
                );
            }
        }
        let backup = previous_bytes(record)?;
        simulated.insert(path.clone(), (backup, record.previous_mode));
    }
    Ok(())
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
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.file_type().is_file() {
            bail!(
                "History checkpoint must be a regular file: {}",
                path.display()
            );
        }
        let data = read_checkpoint_file(&path)
            .with_context(|| format!("Read file checkpoint {}", path.display()))?;
        let record: Record = serde_json::from_slice(&data)
            .with_context(|| format!("Parse file checkpoint {}", path.display()))?;
        if path.file_stem().and_then(|s| s.to_str()) != Some(record.id.to_string().as_str()) {
            bail!(
                "History checkpoint filename/id mismatch: {}",
                path.display()
            )
        }
        let modified = metadata.modified()?;
        results.push((
            modified,
            record.id.to_string(),
            json!({"id":record.id,"path":record.path,"message_id":record.message_id}),
        ));
    }
    results.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    Ok(serde_json::to_string_pretty(
        &results
            .into_iter()
            .map(|(_, _, record)| record)
            .collect::<Vec<_>>(),
    )?)
}
/// Original fileHistory.ts::copyFileHistoryForResume migrates the complete
/// saved backup set when a conversation branches or resumes under a new ID.
/// A hard link avoids copying large files; cross-device storage falls back
/// to copying bytes. The source session is never modified.
pub fn copy_for_resume(root: &Path, previous_session: &str, next_session: &str) -> Result<usize> {
    if previous_session == next_session {
        bail!("History source and destination sessions must differ")
    }
    let source = storage(root, Some(previous_session))?;
    let destination = storage(root, Some(next_session))?;
    if fs::read_dir(&destination)?.next().is_some() {
        bail!("Destination session already contains history checkpoints")
    }
    let operation = (|| -> Result<usize> {
        let mut entries = Vec::new();
        for item in fs::read_dir(&source)? {
            let item = item?;
            let path = item.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let metadata = fs::symlink_metadata(&path)?;
            if !metadata.file_type().is_file() {
                bail!(
                    "History snapshot must be a regular file: {}",
                    path.display()
                )
            }
            let id = path
                .file_stem()
                .and_then(|name| name.to_str())
                .context("History snapshot filename is not valid UTF-8")?;
            let id = Uuid::parse_str(id).context("History snapshot filename must be a UUID")?;
            entries.push((metadata.modified()?, id));
        }
        // Preserve original checkpoint display order after the copy.
        entries.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        for (_, id) in &entries {
            let record = load(root, Some(previous_session), &id.to_string())?;
            if let Some(backup) = record.backup_path.as_ref() {
                let target = destination.join(format!("{id}.bak"));
                // Refuse redirected backup files both before and after
                // linking; hard_link may link the symlink inode itself.
                let _source = open_history_file(backup)?;
                match fs::hard_link(backup, &target) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                        return Err(error).context("History destination backup already exists")
                    }
                    Err(_) => {
                        let mut source_file = open_history_file(backup)?;
                        let mut target_file = fs::OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .open(&target)?;
                        std::io::copy(&mut source_file, &mut target_file)?;
                        target_file.sync_all()?;
                    }
                }
                if !fs::symlink_metadata(&target)?.file_type().is_file() {
                    bail!("History backup copy was redirected: {}", target.display());
                }
            }
            let source_record = source.join(format!("{id}.json"));
            let target_record = destination.join(format!("{id}.json"));
            let mut input = open_history_file(&source_record)?;
            let mut output = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target_record)?;
            std::io::copy(&mut input, &mut output)?;
            output.sync_all()?;
        }
        Ok(entries.len())
    })();
    match operation {
        Ok(count) => Ok(count),
        Err(error) => {
            // The destination ID is new and unpublished. Never leave a
            // partially migrated history that could claim to be complete.
            if let Err(cleanup) = fs::remove_dir_all(&destination) {
                bail!("History copy failed: {error:#}; cleanup failed: {cleanup:#}")
            }
            Err(error)
        }
    }
}

/// Called only if persisting a newly branched conversation fails after its
/// checkpoint copy. The parent session's backup directory remains untouched.
pub fn discard_copied_session(root: &Path, new_session: &str) -> Result<()> {
    let destination = storage(root, Some(new_session))?;
    fs::remove_dir_all(destination)?;
    Ok(())
}

#[derive(Clone)]
pub struct RewindEntry {
    pub path: String,
    before: Option<Vec<u8>>,
    before_mode: Option<u32>,
    after: Option<Vec<u8>>,
    after_mode: Option<u32>,
}

pub struct RewindPlan {
    pub entries: Vec<RewindEntry>,
}

/// Reconstruct a full filesystem state using all later edits. This mirrors
/// fileHistory.ts::applySnapshot (which restores every tracked file), while
/// retaining Rust's existing per-edit checkpoint format and old backups.
/// The caller must authorize every affected path before apply_rewind.
pub fn plan_rewind(root: &Path, session_id: Option<&str>, id: &str) -> Result<RewindPlan> {
    use std::collections::BTreeMap;
    let checkpoint = Uuid::parse_str(id).context("Rewind target must be a UUID")?;
    let folder = storage(root, session_id)?;
    let mut history = Vec::new();
    for entry in fs::read_dir(&folder)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let modified = fs::symlink_metadata(&path)?.modified()?;
        let id = path
            .file_stem()
            .and_then(|name| name.to_str())
            .context("Invalid checkpoint filename")?;
        history.push((modified, id.to_owned()));
    }
    history.sort();
    let target = history
        .iter()
        .position(|(_, current)| current == &checkpoint.to_string())
        .context("Requested file rewind checkpoint was not found")?;
    let mut changes: BTreeMap<String, RewindEntry> = BTreeMap::new();
    for (_, checkpoint_id) in history[target..].iter().rev() {
        let record = load(root, session_id, checkpoint_id)?;
        let path = root.join(&record.path);
        let previous = previous_bytes(&record)?;
        let current = snapshot(&path)?;
        if let Some(entry) = changes.get_mut(&record.path) {
            if entry.after.as_ref().map(|bytes| fingerprint(bytes))
                != Some(record.updated_hash.clone())
            {
                bail!(
                    "File history diverged between checkpoints for {}",
                    record.path
                );
            }
            entry.after = previous;
            entry.after_mode = record.previous_mode;
        } else {
            if current
                .as_ref()
                .is_some_and(|bytes| fingerprint(bytes) != record.updated_hash)
            {
                bail!("File changed after its last checkpoint: {}", record.path);
            }
            let entry = RewindEntry {
                path: record.path.clone(),
                before: current,
                before_mode: snapshot_mode(&path)?,
                after: previous,
                after_mode: record.previous_mode,
            };
            changes.insert(record.path, entry);
        }
    }
    Ok(RewindPlan {
        entries: changes.into_values().collect(),
    })
}

fn publish_rewind_entry(path: &Path, bytes: Option<&[u8]>, mode: Option<u32>) -> Result<()> {
    if let Some(bytes) = bytes {
        publish_verified_restore(path, bytes, mode)
    } else {
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

/// All changes are validated before the first write. If any subsequent write
/// fails, restore already changed files instead of leaving a half-rewound turn.
pub fn apply_rewind(root: &Path, plan: &RewindPlan) -> Result<String> {
    for entry in &plan.entries {
        let actual = snapshot(&root.join(&entry.path))?;
        if actual != entry.before {
            bail!("File changed during rewind planning: {}", entry.path);
        }
    }
    let mut committed: Vec<&RewindEntry> = Vec::new();
    for entry in &plan.entries {
        if let Err(error) = publish_rewind_entry(
            &root.join(&entry.path),
            entry.after.as_deref(),
            entry.after_mode,
        ) {
            let mut failures = Vec::new();
            for done in committed.into_iter().rev() {
                if let Err(rollback) = publish_rewind_entry(
                    &root.join(&done.path),
                    done.before.as_deref(),
                    done.before_mode,
                ) {
                    failures.push(format!("{}: {rollback:#}", done.path));
                }
            }
            if failures.is_empty() {
                return Err(error).context("Multi-file rewind rolled back");
            }
            bail!(
                "Multi-file rewind failed: {error:#}; rollback failures: {}",
                failures.join("; ")
            );
        }
        committed.push(entry);
    }
    Ok(format!(
        "Restored {} files to checkpoint state",
        plan.entries.len()
    ))
}

pub fn definitions() -> Vec<Value> {
    vec![
        json!({"type":"function","function":{"name":"file_history",
            "description":"List local file checkpoints generated by file write, edit or notebook edits.",
            "parameters":{"type":"object","properties":{}}}}),
        json!({"type":"function","function":{"name":"file_history_diff",
            "description":"Preview line additions/deletions and whether a checkpoint can be safely restored without modifying files.",
            "parameters":{"type":"object","properties":{"id":{"type":"string"}},"required":["id"]}}}),
        json!({"type":"function","function":{"name":"file_rewind",
            "description":"Rewind all tracked files changed since a selected checkpoint, after validating the complete multi-file state and approval.",
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
    fn rewind_applies_all_later_file_edits_and_rejects_external_changes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let first = root.join("first.txt");
        let second = root.join("second.txt");
        fs::write(&first, "first-old").unwrap();
        fs::write(&second, "second-old").unwrap();
        let old_first = snapshot(&first).unwrap();
        fs::write(&first, "first-new").unwrap();
        let first_id = record(root, None, &first, old_first, None)
            .unwrap()
            .unwrap();
        let old_second = snapshot(&second).unwrap();
        fs::write(&second, "second-new").unwrap();
        record(root, None, &second, old_second, None)
            .unwrap()
            .unwrap();
        let plan = plan_rewind(root, None, &first_id.to_string()).unwrap();
        assert_eq!(plan.entries.len(), 2);
        fs::write(&second, "outside modification").unwrap();
        assert!(apply_rewind(root, &plan).is_err());
        assert_eq!(fs::read_to_string(&first).unwrap(), "first-new");
        fs::write(&second, "second-new").unwrap();
        apply_rewind(root, &plan).unwrap();
        assert_eq!(fs::read_to_string(&first).unwrap(), "first-old");
        assert_eq!(fs::read_to_string(&second).unwrap(), "second-old");
    }
    #[test]
    fn checkpoints_restore_changes_and_refuse_stale_revert() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("file.txt");
        fs::write(&source, "old").unwrap();
        let before = snapshot(&source).unwrap();
        fs::write(&source, "new").unwrap();
        let id = record(root.path(), None, &source, before, None)
            .unwrap()
            .unwrap();
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
        let id = record(root.path(), None, &created, before, None)
            .unwrap()
            .unwrap();
        restore(&load(root.path(), None, &id.to_string()).unwrap(), &created).unwrap();
        assert!(!created.exists());
    }
}
