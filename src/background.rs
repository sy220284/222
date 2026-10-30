use crate::{task_guards, task_output_format};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::Command,
    sync::{oneshot, Mutex as AsyncMutex, Notify},
    time::timeout,
};
use uuid::Uuid;

// Original diskOutput.ts caps the combined task output at 5 GiB.
// Keep the cap in bytes (u64), not characters, to preserve large outputs.
const LOG_CAP: u64 = 5 * 1024 * 1024 * 1024;
const OUTPUT_CAPPED_NOTICE: &str = "\n[output truncated: exceeded 5GB disk cap]\n";
const DISPLAY_CAP: usize = 8 * 1024 * 1024;

// Task.ts::generateTaskId uses a task-type prefix and eight lowercase
// alphanumeric characters. Continue accepting UUID IDs created by earlier
// Rust builds so saved task output remains accessible after upgrading.
fn canonical_shell_task_id(id: &str) -> Result<String> {
    if let Ok(uuid) = Uuid::parse_str(id) {
        return Ok(uuid.to_string());
    }
    let bytes = id.as_bytes();
    if bytes.len() == 9
        && bytes[0] == b'b'
        && bytes[1..]
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
    {
        return Ok(id.to_owned());
    }
    bail!("Invalid background task ID")
}

fn new_shell_task_id() -> String {
    const ALPHABET: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let random = Uuid::new_v4();
    let mut id = String::with_capacity(9);
    id.push('b');
    for byte in random.as_bytes().iter().take(8) {
        id.push(ALPHABET[(*byte as usize) % ALPHABET.len()] as char);
    }
    id
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
    pub id: String,
    pub description: String,
    pub command: String,
    pub status: String,
    pub exit_code: Option<i32>,
    pub output_file: String,
    pub error_file: String,
    #[serde(default)]
    pub truncated: bool,
}

struct Running {
    record: Mutex<TaskRecord>,
    stop: Mutex<Option<oneshot::Sender<()>>>,
    persistence_warning: Mutex<Option<String>>,
    completed: Notify,
}

// One ordered output stream and disk budget for both shell pipes.
struct CombinedLog {
    file: tokio::fs::File,
    size: u64,
    capped: bool,
}

/// The original diskOutput.ts stores task streams under a per-project,
/// per-session temporary directory, outside the source checkout. Keeping
/// stdout/stderr in .atlas/background risks indexing and committing them.
fn private_task_directory(root: &Path, session_id: Uuid) -> Result<PathBuf> {
    use sha2::{Digest, Sha256};
    let project = root.canonicalize().context("Workspace unavailable")?;
    let temp = std::env::temp_dir().canonicalize().context("Temporary directory unavailable")?;
    #[cfg(unix)]
    let owner = format!("atlas-agent-{}", unsafe { libc::geteuid() });
    #[cfg(not(unix))]
    let owner = {
        let home = dirs::home_dir().context("Cannot identify user for private task output")?;
        let hash = format!("{:x}", Sha256::digest(home.to_string_lossy().as_bytes()));
        format!("atlas-agent-{}", &hash[..24])
    };
    let project_hash = format!("{:x}", Sha256::digest(project.to_string_lossy().as_bytes()));
    let mut path = temp;
    for component in [
        owner,
        format!("project-{}", &project_hash[..32]),
        session_id.to_string(),
        "tasks".to_owned(),
    ] {
        path.push(component);
        match fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => {
                bail!("Task output directory is redirected or invalid: {}", path.display())
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if let Err(error) = fs::create_dir(&path) {
                    if error.kind() != std::io::ErrorKind::AlreadyExists {
                        return Err(error).with_context(|| format!(
                            "Create task output directory {}", path.display()
                        ));
                    }
                }
                let meta = fs::symlink_metadata(&path)?;
                if meta.file_type().is_symlink() || !meta.is_dir() {
                    bail!("Task output directory was redirected: {}", path.display())
                }
            }
            Err(error) => return Err(error.into()),
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            let meta = fs::symlink_metadata(&path)?;
            if meta.file_type().is_symlink() || !meta.is_dir()
                || meta.uid() != unsafe { libc::geteuid() }
            {
                bail!("Task output directory is not owned by this user: {}", path.display())
            }
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
        }
    }
    path.canonicalize().context("Private task output directory unavailable")
}

pub struct BackgroundTasks {
    workspace: PathBuf,
    directory: PathBuf,
    entries: Mutex<HashMap<String, Arc<Running>>>,
    shell_kind: Mutex<crate::shell_runtime::ShellKind>,
    session_env: Mutex<std::collections::BTreeMap<String, String>>,
}

impl BackgroundTasks {
    pub fn new(root: &Path) -> Result<Self> {
        Self::in_directory(root, root.join(".atlas").join("background"))
    }

    /// Match diskOutput.ts::getTaskOutputDir: each conversation owns its
    /// background output. A second session in the same workspace must never
    /// list, stop, or read the first session's live shell tasks.
    pub fn for_session(root: &Path, session_id: Uuid) -> Result<Self> {
        // diskOutput.ts::getTaskOutputDir uses a private project temporary
        // directory with a stable session ID. Do not put task output into
        // the repository or regenerate the location during /clear.
        // Existing Rust sessions kept their task metadata in the workspace.
        // Preserve access to those records until the session ends instead
        // of silently losing completed output during an upgrade.
        let legacy = root.join(".atlas").join("background").join(session_id.to_string());
        match fs::symlink_metadata(&legacy) {
            Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => {
                bail!("Previous task output directory is redirected or invalid")
            }
            Ok(_) => {
                let mut has_records = false;
                for entry in fs::read_dir(&legacy)? {
                    let entry = entry?;
                    if entry.path().extension().and_then(|ext| ext.to_str()) == Some("json") {
                        let metadata = fs::symlink_metadata(entry.path())?;
                        if metadata.file_type().is_symlink() || !metadata.is_file()
                            || metadata.len() > 1024 * 1024
                        {
                            bail!("Previous task metadata is redirected or invalid: {}",
                                entry.path().display());
                        }
                        let stem = entry.path().file_stem()
                            .and_then(|stem| stem.to_str())
                            .context("Previous task metadata filename is invalid")?
                            .to_owned();
                        if canonical_shell_task_id(&stem).is_err() {
                            continue;
                        }
                        let record: TaskRecord = serde_json::from_slice(&fs::read(entry.path())?)
                            .with_context(|| format!(
                                "Read previous task metadata {}", entry.path().display()
                            ))?;
                        if record.id != stem {
                            bail!("Previous task metadata ID mismatch: {}", entry.path().display());
                        }
                        has_records = true;
                    }
                }
                if has_records {
                    return Self::in_directory(root, legacy);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let directory = private_task_directory(root, session_id)?;
        Self::in_directory_checked(root, directory, true)
    }

    fn in_directory(root: &Path, directory: PathBuf) -> Result<Self> {
        Self::in_directory_checked(root, directory, false)
    }

    fn in_directory_checked(root: &Path, directory: PathBuf, private: bool) -> Result<Self> {
        let workspace = root.canonicalize().context("Workspace unavailable")?;
        if !private {
            fs::create_dir_all(&directory)?;
        }
        let canonical = directory.canonicalize()?;
        if private {
            if !canonical.starts_with(std::env::temp_dir().canonicalize()?) {
                bail!("Private background task directory escapes temporary storage");
            }
        } else if !canonical.starts_with(&workspace) {
            bail!("Background task directory escapes workspace");
        }
        Ok(Self {
            workspace,
            directory: canonical,
            entries: Mutex::new(HashMap::new()),
            shell_kind: Mutex::new(crate::shell_runtime::ShellKind::default()),
            session_env: Mutex::new(std::collections::BTreeMap::new()),
        })
    }

    /// FileReadTool may read a task's own output under the original
    /// project temporary directory. This is a narrow read-only exception:
    /// never authorize another session's file or a symlinked output.
    pub fn trusted_output_path(&self, raw: &str) -> Result<Option<PathBuf>> {
        let path = Path::new(raw);
        if !path.is_absolute() {
            return Ok(None);
        }
        let Some(parent) = path.parent() else {
            return Ok(None);
        };
        if parent != self.directory {
            // filesystem.ts::checkReadableInternalPath allows the project
            // temporary directory across sessions. Limit this exception
            // to a sibling UUID session's tasks directory, never to an
            // arbitrary temp path or another project.
            let Some(project_dir) = self.directory.parent().and_then(Path::parent) else {
                return Ok(None);
            };
            let Some(session_dir) = parent.parent() else {
                return Ok(None);
            };
            if session_dir.parent() != Some(project_dir)
                || parent.file_name().and_then(|name| name.to_str()) != Some("tasks")
                || !session_dir.file_name().and_then(|name| name.to_str())
                    .is_some_and(|name| Uuid::parse_str(name).is_ok())
            {
                return Ok(None);
            }
            for directory in [session_dir, parent] {
                let meta = fs::symlink_metadata(directory)?;
                if meta.file_type().is_symlink() || !meta.is_dir() {
                    bail!("Task output directory is redirected or invalid");
                }
            }
            if parent.canonicalize()? != parent {
                bail!("Task output directory is redirected");
            }
        }
        if !matches!(path.extension().and_then(|ext| ext.to_str()), Some("stdout" | "stderr" | "output")) {
            return Ok(None);
        }
        let Some(id) = path.file_stem().and_then(|stem| stem.to_str()) else {
            return Ok(None);
        };
        if canonical_shell_task_id(id).ok().as_deref() != Some(id) {
            return Ok(None);
        }
        // A sibling session's output belongs to that sibling's metadata.
        // Resolve the record next to the validated output, not in the
        // reader's session, so project-local Read works across sessions.
        let record = parent.join(format!("{id}.json"));
        let record_meta = fs::symlink_metadata(&record)
            .with_context(|| format!("Inspect task record {}", record.display()))?;
        if record_meta.file_type().is_symlink() || !record_meta.is_file() {
            bail!("Task record is redirected or invalid");
        }
        let output_meta = fs::symlink_metadata(path)
            .with_context(|| format!("Inspect task output {}", path.display()))?;
        if output_meta.file_type().is_symlink() || !output_meta.is_file() {
            bail!("Task output is redirected or invalid");
        }
        Ok(Some(path.to_path_buf()))
    }

    pub fn set_environment(&self, variables: &std::collections::BTreeMap<String, String>) {
        *self.session_env.lock().unwrap() = variables.clone();
    }
    pub fn set_shell(&self, kind: crate::shell_runtime::ShellKind) {
        *self.shell_kind.lock().unwrap() = kind;
    }
    fn metadata_path(&self, id: &str) -> Result<PathBuf> {
        let id = canonical_shell_task_id(id)?;
        Ok(self.directory.join(format!("{id}.json")))
    }

    fn save(directory: &Path, record: &TaskRecord) -> Result<()> {
        use std::io::Write;
        let path = directory.join(format!("{}.json", record.id));
        let temp = directory.join(format!("{}.{}.tmp", record.id, Uuid::new_v4()));
        // Never follow an attacker-created temporary-file symlink while
        // persisting task state. The metadata rename is atomic.
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW);
        }
        let mut file = options.open(&temp)?;
        let write_result = (|| -> Result<()> {
            file.write_all(&serde_json::to_vec_pretty(record)?)?;
            file.sync_all()?;
            Ok(())
        })();
        drop(file);
        if let Err(error) = write_result {
            let _ = fs::remove_file(&temp);
            return Err(error);
        }
        if let Err(error) = fs::rename(&temp, &path) {
            let _ = fs::remove_file(&temp);
            return Err(error.into());
        }
        Ok(())
    }

    fn load(&self, id: &str) -> Result<TaskRecord> {
        use std::io::Read;
        let path = self.metadata_path(id)?;
        let mut options = fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW);
        }
        let mut file = options.open(path)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.len() > 1024 * 1024 {
            bail!("Background task metadata must be a regular file under 1 MiB")
        }
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        file.read_to_end(&mut bytes)?;
        let mut record: TaskRecord = serde_json::from_slice(&bytes)?;
        if record.id != id {
            bail!("Task metadata ID mismatch");
        }
        // Spawned processes are bound to their owning agent process.
        // A stale 'running' record must not masquerade as a live task.
        if record.status == "running" {
            record.status = "interrupted".to_owned();
            Self::save(&self.directory, &record)?;
        }
        Ok(record)
    }

    pub async fn start(&self, command: &str, description: &str) -> Result<String> {
        let kind = *self.shell_kind.lock().unwrap();
        self.start_with_shell(command, description, kind).await
    }

    /// Explicit tool names select their own shell without changing the
    /// session default or racing another background task's shell choice.
    pub async fn start_with_shell(
        &self,
        command: &str,
        description: &str,
        kind: crate::shell_runtime::ShellKind,
    ) -> Result<String> {
        if command.trim().is_empty() {
            bail!("Background shell command is empty")
        }
        if command.len() > 100_000 {
            bail!("Background shell command too large")
        }
        let id = (0..32)
            .map(|_| new_shell_task_id())
            .find(|candidate| {
                !self.directory.join(format!("{candidate}.json")).exists()
                    && !self.directory.join(format!("{candidate}.stdout")).exists()
                    && !self.directory.join(format!("{candidate}.stderr")).exists()
                    && !self.directory.join(format!("{candidate}.output")).exists()
            })
            .context("Could not allocate a unique background task ID")?;
        // Validate the shell command before reserving output files: a
        // command-construction failure must not leave stale task artifacts.
        let mut proc = kind.build_command(command)?;
        proc.envs(self.session_env.lock().unwrap().iter());
        crate::subprocess_env::apply(&mut proc);
        let stdout_path = self.directory.join(format!("{id}.stdout"));
        let stderr_path = self.directory.join(format!("{id}.stderr"));
        let output_path = self.directory.join(format!("{id}.output"));
        // Hold the descriptors from exclusive creation through spooling: never
        // reopen by path. This closes the symlink substitution race.
        let stdout_file = create_output_file(&stdout_path)?;
        let stderr_file = match create_output_file(&stderr_path) {
            Ok(file) => file,
            Err(error) => {
                let _ = fs::remove_file(&stdout_path);
                return Err(error);
            }
        };
        let output_file = match create_output_file(&output_path) {
            Ok(file) => file,
            Err(error) => {
                drop(stdout_file);
                drop(stderr_file);
                let _ = fs::remove_file(&stdout_path);
                let _ = fs::remove_file(&stderr_path);
                return Err(error);
            }
        };
        // Output directories are session-scoped; shell execution must still
        // run in the actual project root, never inside .atlas/background.
        let workspace = &self.workspace;
        // Place the shell and its descendants into a new process group.
        // Stopping only the shell can otherwise leave children holding pipes.
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(proc.as_std_mut(), 0);
        proc.current_dir(workspace)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = match proc.spawn() {
            Ok(child) => child,
            Err(error) => {
                // No process was started; discard its reserved output files.
                drop(stdout_file);
                drop(stderr_file);
                drop(output_file);
                let _ = fs::remove_file(&stdout_path);
                let _ = fs::remove_file(&stderr_path);
                let _ = fs::remove_file(&output_path);
                return Err(error).context("Cannot start background command");
            }
        };
        let stdout = child.stdout.take().context("No background stdout")?;
        let stderr = child.stderr.take().context("No background stderr")?;
        let record = TaskRecord {
            id: id.clone(),
            description: description.to_owned(),
            command: command.to_owned(),
            status: "running".into(),
            exit_code: None,
            output_file: output_path.to_string_lossy().to_string(),
            error_file: stderr_path.to_string_lossy().to_string(),
            truncated: false,
        };
        if let Err(error) = Self::save(&self.directory, &record) {
            // A started shell must not survive an unsuccessful task registration.
            // Otherwise the user receives an error but the command keeps running.
            #[cfg(unix)]
            if let Some(pid) = child.id() {
                let _ = unsafe { libc::kill(-(pid as i32), libc::SIGKILL) };
            }
            let _ = child.kill().await;
            let _ = child.wait().await;
            drop(stdout);
            drop(stderr);
            drop(output_file);
            let _ = fs::remove_file(&stdout_path);
            let _ = fs::remove_file(&stderr_path);
            let _ = fs::remove_file(&output_path);
            return Err(error).context("Persist background task registration");
        }
        let (stop, stopped) = oneshot::channel();
        let running = Arc::new(Running {
            record: Mutex::new(record.clone()),
            stop: Mutex::new(Some(stop)),
            persistence_warning: Mutex::new(None),
            completed: Notify::new(),
        });
        self.entries
            .lock()
            .unwrap()
            .insert(id.clone(), running.clone());
        let folder = self.directory.clone();
        tokio::spawn(async move {
            let merged = Arc::new(AsyncMutex::new(CombinedLog {
                file: output_file, size: 0, capped: false,
            }));
            let stdout_task = tokio::spawn(spool_merged(stdout, stdout_file, merged.clone()));
            let stderr_task = tokio::spawn(spool_merged(stderr, stderr_file, merged.clone()));
            let (mut state, code) = tokio::select! {
                result = child.wait() => match result {
                    Ok(status) if status.success() => ("completed", status.code()),
                    Ok(status) => ("failed", status.code()),
                    Err(_) => ("failed", None),
                },
                _ = stopped => {
                    #[cfg(unix)]
                    if let Some(pid) = child.id() {
                        // A negative PID targets the entire process group,
                        // including Shell children which inherited output pipes.
                        let _ = unsafe { libc::kill(-(pid as i32), libc::SIGKILL) };
                    }
                    let _ = child.kill().await;
                    ("killed", None)
                }
            };
            let (out, stdout_failed) = match stdout_task.await {
                Ok(Ok(capped)) => (capped, false),
                Ok(Err(error)) => {
                    eprintln!("Background stdout persistence failed: {error:#}");
                    (true, true)
                }
                Err(error) => {
                    eprintln!("Background stdout writer failed: {error}");
                    (true, true)
                }
            };
            let (err, stderr_failed) = match stderr_task.await {
                Ok(Ok(capped)) => (capped, false),
                Ok(Err(error)) => {
                    eprintln!("Background stderr persistence failed: {error:#}");
                    (true, true)
                }
                Err(error) => {
                    eprintln!("Background stderr writer failed: {error}");
                    (true, true)
                }
            };
            let merged_failed = {
                let mut log = merged.lock().await;
                if let Err(error) = log.file.flush().await {
                    eprintln!("Background combined output persistence failed: {error:#}");
                    true
                } else {
                    false
                }
            };
            if stdout_failed || stderr_failed || merged_failed {
                // A successful shell exit with lost output is a failed task,
                // not a silently successful command with incomplete evidence.
                if state == "completed" {
                    state = "failed";
                }
            }
            let updated = {
                let mut state_record = running.record.lock().unwrap();
                state_record.status = state.into();
                state_record.exit_code = code;
                state_record.truncated = out || err;
                state_record.clone()
            };
            if let Err(error) = Self::save(&folder, &updated) {
                let warning = format!("Task completed but final state could not be saved: {error:#}");
                eprintln!("{warning}");
                *running.persistence_warning.lock().unwrap() = Some(warning);
            }
            running.completed.notify_waiters();
        });
        Ok(
            json!({"task_id":id,"task_type":"local_bash","status":"running",
            "output_file":record.output_file,"error_file":record.error_file})
            .to_string(),
        )
    }

    pub async fn output(&self, id: &str, block: bool, timeout_ms: u64) -> Result<String> {
        let running = self.entries.lock().unwrap().get(id).cloned();
        let record = if let Some(task) = running {
            let wait = async {
                loop {
                    let notified = task.completed.notified();
                    let current = task.record.lock().unwrap().clone();
                    if current.status != "running" {
                        return current;
                    }
                    notified.await;
                }
            };
            if block {
                match timeout(Duration::from_millis(timeout_ms.min(600_000)), wait).await {
                    Ok(value) => value,
                    Err(_) => task.record.lock().unwrap().clone(),
                }
            } else {
                task.record.lock().unwrap().clone()
            }
        } else {
            self.load(id)?
        };
        // Do not trust paths in persisted task metadata: a workspace file
        // must never redirect output reads to a private host file.
        let task_id = canonical_shell_task_id(id)?;
        let safe_stdout = self.directory.join(format!("{task_id}.stdout"));
        let safe_stderr = self.directory.join(format!("{task_id}.stderr"));
        let combined = self.directory.join(format!("{task_id}.output"));
        let (text, result_path) = match fs::symlink_metadata(&combined) {
            Ok(meta) if meta.is_file() && !meta.file_type().is_symlink() => {
                (tail(combined.to_str().context("Invalid output path")?)?, combined)
            }
            Ok(_) => bail!("Combined task output is redirected or invalid"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                // Older task records stored stdout/stderr separately.
                let stdout = tail(safe_stdout.to_str().context("Invalid stdout path")?)?;
                let stderr = tail(safe_stderr.to_str().context("Invalid stderr path")?)?;
                let legacy = [stdout, stderr].into_iter()
                    .filter(|part| !part.is_empty()).collect::<Vec<_>>().join("\n");
                (legacy, safe_stdout)
            }
            Err(error) => return Err(error.into()),
        };
        let displayed = task_output_format::format_task_output(
            &text,
            result_path.to_str().context("Invalid task output path")?,
            task_output_format::max_output_length(),
        );
        let retrieval = if record.status == "running" {
            if block {
                "timeout"
            } else {
                "not_ready"
            }
        } else {
            "success"
        };
        debug_assert!(task_guards::is_local_shell_task(
            &json!({"type":"local_bash"})
        ));
        Ok(json!({"retrieval_status":retrieval,"task":{
            "task_id":record.id,"task_type":"local_bash",
            "status":record.status,"description":record.description,
            "output":displayed.content,"exitCode":record.exit_code,
            "output_file":result_path.to_string_lossy(),
            "persistenceWarning":self.entries.lock().unwrap().get(id)
                .and_then(|task| task.persistence_warning.lock().unwrap().clone()),
            "truncated":record.truncated || displayed.was_truncated
        }})
        .to_string())
    }

    /// Read only newly appended stdout bytes, using the original task
    /// framework's offset/delta contract. A stopped task's file remains
    /// readable across agent restarts. Never trust persisted path metadata.
    pub fn output_delta(&self, id: &str, from_offset: u64, max_bytes: u64) -> Result<String> {
        let id = canonical_shell_task_id(id)?;
        let combined = self.directory.join(format!("{id}.output"));
        let path = match fs::symlink_metadata(&combined) {
            Ok(meta) if meta.is_file() && !meta.file_type().is_symlink() => combined,
            Ok(_) => bail!("Combined task output is redirected or invalid"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                self.directory.join(format!("{id}.stdout"))
            }
            Err(error) => return Err(error.into()),
        };
        // diskOutput.ts::getTaskOutputDelta applies the 8 MiB default only
        // when maxBytes is omitted. An explicit larger range is respected.
        let delta = read_output_range(&path, from_offset, max_bytes)?;
        let (content, bytes_read, total) = match delta {
            Some((content, bytes_read, total)) => (content, bytes_read, total),
            None => (String::new(), 0, 0),
        };
        Ok(json!({
            "task_id":id,
            "content":content,
            "newOffset":from_offset.saturating_add(bytes_read),
            "bytesRead":bytes_read,
            "bytesTotal":total,
            "output_file":path.to_string_lossy(),
        })
        .to_string())
    }

    pub fn list(&self) -> Result<String> {
        let live = self.entries.lock().unwrap().clone();
        let mut records = Vec::new();
        for entry in fs::read_dir(&self.directory)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if canonical_shell_task_id(id).is_err() {
                continue;
            }
            let record = if let Some(running) = live.get(id) {
                running.record.lock().unwrap().clone()
            } else {
                match self.load(id) {
                    Ok(r) => r,
                    Err(_) => continue,
                }
            };
            records.push(json!({
                "task_id":record.id,
                "task_type":"local_bash",
                "description":record.description,
                "status":record.status,
                "exitCode":record.exit_code,
                "persistenceWarning":live.get(id)
                    .and_then(|task| task.persistence_warning.lock().unwrap().clone())
            }));
        }
        records.sort_by(|a, b| a["task_id"].as_str().cmp(&b["task_id"].as_str()));
        Ok(serde_json::to_string_pretty(&records)?)
    }

    /// TaskStopTool validates the target before asking for permission. This
    /// is a read-only preflight; stop() rechecks after approval to handle races.
    pub fn validate_stop(&self, id: &str) -> Result<()> {
        let id = canonical_shell_task_id(id)?;
        if let Some(task) = self.entries.lock().unwrap().get(&id).cloned() {
            let status = task.record.lock().unwrap().status.clone();
            if status == "running" {
                return Ok(());
            }
            bail!("Task {id} is not running (status: {status})");
        }
        match self.load(&id) {
            Ok(record) => bail!("Task {id} is not running (status: {})", record.status),
            Err(_) => bail!("No task found with ID: {id}"),
        }
    }

    pub async fn stop(&self, id: &str) -> Result<String> {
        let task = match self.entries.lock().unwrap().get(id).cloned() {
            Some(task) => task,
            None => match self.load(id) {
                Ok(record) => bail!("Task {id} is not running (status: {})", record.status),
                Err(_) => bail!("No task found with ID: {id}"),
            },
        };
        let record = task.record.lock().unwrap().clone();
        if record.status != "running" {
            bail!("Task {id} is not running (status: {})", record.status);
        }
        let sender = task
            .stop
            .lock()
            .unwrap()
            .take()
            .context("Task stop already requested")?;
        sender
            .send(())
            .map_err(|_| anyhow::anyhow!("Task already completed"))?;
        timeout(Duration::from_secs(5), async {
            loop {
                let notified = task.completed.notified();
                if task.record.lock().unwrap().status != "running" {
                    break;
                }
                notified.await;
            }
        })
        .await
        .context("Timed out waiting for background shell to stop")?;
        let final_status = task.record.lock().unwrap().status.clone();
        if final_status != "killed" {
            bail!("Task {id} finished before stop took effect (status: {final_status})");
        }
        Ok(json!({"task_id":id,"task_type":"local_bash",
            "command":record.command,
            "message":format!("Successfully stopped task: {id} ({})", record.command)
        })
        .to_string())
    }

    pub async fn stop_all(&self) {
        let tasks: Vec<_> = self.entries.lock().unwrap().values().cloned().collect();
        // Send cancellation to every task first. Waiting three seconds per
        // task serially made session shutdown take minutes with many shells.
        for task in &tasks {
            if task.record.lock().unwrap().status == "running" {
                if let Some(sender) = task.stop.lock().unwrap().take() {
                    let _ = sender.send(());
                }
            }
        }
        // All child processes can now stop concurrently. Use one shared
        // shutdown deadline rather than resetting the clock for each task.
        let _ = timeout(Duration::from_secs(3), async {
            for task in tasks {
                loop {
                    let notified = task.completed.notified();
                    if task.record.lock().unwrap().status != "running" {
                        break;
                    }
                    notified.await;
                }
            }
        })
        .await;
    }
}

fn read_output_range(
    path: &Path,
    offset: u64,
    max_bytes: u64,
) -> Result<Option<(String, u64, u64)>> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = match options.open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let meta = file.metadata()?;
    if !meta.is_file() {
        bail!("Task output is not a regular file")
    }
    let total = meta.len();
    if offset >= total {
        return Ok(Some((String::new(), 0, total)));
    }
    use std::io::{Read, Seek, SeekFrom};
    file.seek(SeekFrom::Start(offset))?;
    let bytes_to_read = (total - offset).min(max_bytes);
    let capacity = usize::try_from(bytes_to_read)
        .context("Requested task output range exceeds addressable memory")?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(capacity)
        .context("Cannot reserve memory for requested task output range")?;
    file.take(bytes_to_read).read_to_end(&mut bytes)?;
    let count = bytes.len() as u64;
    Ok(Some((
        String::from_utf8_lossy(&bytes).into_owned(),
        count,
        total,
    )))
}

fn tail(path: &str) -> Result<String> {
    // diskOutput.ts::getTaskOutput returns an empty transcript when an output
    // file has already been evicted or was never created. Keep the task's
    // status readable after a restart instead of failing the whole tool.
    match tail_with_limit(Path::new(path), DISPLAY_CAP as u64) {
        Ok(content) => Ok(content),
        Err(error)
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|io| io.kind() == std::io::ErrorKind::NotFound) =>
        {
            Ok(String::new())
        }
        Err(error) => Err(error),
    }
}

fn tail_with_limit(path: &Path, max_bytes: u64) -> Result<String> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options.open(path)?;
    if !file.metadata()?.is_file() {
        bail!("Task output path is not a regular file");
    }
    let len = file.metadata()?.len();
    let start = len.saturating_sub(max_bytes);
    use std::io::{Read, Seek, SeekFrom};
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    file.take(max_bytes).read_to_end(&mut bytes)?;
    let earlier = len.saturating_sub(bytes.len() as u64);
    let content = String::from_utf8_lossy(&bytes);
    if earlier == 0 {
        Ok(content.into_owned())
    } else {
        Ok(format!(
            "[{}KB of earlier output omitted]\n{content}",
            (earlier as f64 / 1024.0).round()
        ))
    }
}

fn create_output_file(path: &Path) -> Result<tokio::fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let file = options
        .open(path)
        .with_context(|| format!("Securely creating background output {}", path.display()))?;
    Ok(tokio::fs::File::from_std(file))
}

async fn spool<R: AsyncRead + Unpin>(stream: R, file: tokio::fs::File) -> Result<bool> {
    spool_with_limit(stream, file, LOG_CAP).await
}

async fn spool_with_limit<R: AsyncRead + Unpin>(
    mut stream: R,
    mut file: tokio::fs::File,
    limit: u64,
) -> Result<bool> {
    let mut buffer = [0u8; 8192];
    let mut size = 0u64;
    let mut truncated = false;
    loop {
        let n = stream.read(&mut buffer).await?;
        if n == 0 {
            break;
        }
        let writable = n.min(limit.saturating_sub(size).min(n as u64) as usize);
        if writable > 0 {
            file.write_all(&buffer[..writable]).await?;
            size += writable as u64;
        }
        if writable < n && !truncated {
            truncated = true;
            // Keep consuming the pipe so the child does not deadlock, but
            // append the original disk-cap diagnostic exactly once.
            file.write_all(OUTPUT_CAPPED_NOTICE.as_bytes()).await?;
        }
    }
    file.flush().await?;
    Ok(truncated)
}

// Append both pipes to one transcript in chunk arrival order, using a
// shared 5 GiB cap. Keep the separate files for existing diagnostics.
async fn spool_merged<R: AsyncRead + Unpin>(
    mut stream: R,
    mut individual: tokio::fs::File,
    combined: Arc<AsyncMutex<CombinedLog>>,
) -> Result<bool> {
    let mut buffer = [0u8; 8192];
    let mut truncated = false;
    loop {
        let n = stream.read(&mut buffer).await?;
        if n == 0 { break; }
        let mut shared = combined.lock().await;
        let writable = n.min(LOG_CAP.saturating_sub(shared.size).min(n as u64) as usize);
        if writable > 0 {
            shared.file.write_all(&buffer[..writable]).await?;
            individual.write_all(&buffer[..writable]).await?;
            shared.size += writable as u64;
        }
        if writable < n {
            truncated = true;
            if !shared.capped {
                shared.file.write_all(OUTPUT_CAPPED_NOTICE.as_bytes()).await?;
                individual.write_all(OUTPUT_CAPPED_NOTICE.as_bytes()).await?;
                shared.capped = true;
            }
        }
    }
    individual.flush().await?;
    Ok(truncated)
}

pub fn definitions() -> Vec<Value> {
    vec![
        json!({"type":"function","function":{"name":"background_list",
            "description":"List background shell tasks, including previously completed or interrupted jobs.",
            "parameters":{"type":"object","properties":{}}}}),
        json!({"type":"function","function":{"name":"task_output",
            "description":"Return status and saved output for a background shell task. block=true waits up to timeout milliseconds.",
            "parameters":{"type":"object","properties":{
                "task_id":{"type":"string"},"block":{"type":"boolean"},
                "timeout":{"type":"integer","minimum":0,"maximum":600000}
            },"required":["task_id"]}}}),
        json!({"type":"function","function":{"name":"task_output_delta",
            "description":"Read appended stdout by byte offset; defaults to 8MiB, but honors explicit larger ranges; never follows a symlink.",
            "parameters":{"type":"object","properties":{
                "task_id":{"type":"string"},"from_offset":{"type":"integer","minimum":0},
                "max_bytes":{"type":"integer","minimum":0,"maximum":5368709120}
            },"required":["task_id"]}}}),
        json!({"type":"function","function":{"name":"task_stop",
        "description":"Stop a running background shell task by ID. Requires approval.",
        "parameters":{"type":"object","properties":{
            "task_id":{"type":"string"},
            "shell_id":{"type":"string","description":"Legacy alias for task_id"}
        }}}}),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn project_sibling_session_task_logs_are_readable_without_crossing_projects() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path().canonicalize().unwrap();
        let source = BackgroundTasks::for_session(&root, Uuid::new_v4()).unwrap();
        let reader = BackgroundTasks::for_session(&root, Uuid::new_v4()).unwrap();
        let id = new_shell_task_id();
        let output = source.directory.join(format!("{id}.stdout"));
        let error = source.directory.join(format!("{id}.stderr"));
        fs::write(&output, b"output from sibling session").unwrap();
        fs::write(&error, b"").unwrap();
        BackgroundTasks::save(&source.directory, &TaskRecord {
            id: id.clone(), description: "Previous session".into(),
            command: "printf example".into(), status: "completed".into(),
            exit_code: Some(0), output_file: output.to_string_lossy().into_owned(),
            error_file: error.to_string_lossy().into_owned(), truncated: false,
        }).unwrap();
        assert_eq!(
            reader.trusted_output_path(output.to_str().unwrap()).unwrap(),
            Some(output.clone()),
        );
        assert_eq!(
            reader.trusted_output_path(error.to_str().unwrap()).unwrap(),
            Some(error),
        );
        let other_project = tempfile::tempdir().unwrap();
        let outsider = BackgroundTasks::for_session(
            &other_project.path().canonicalize().unwrap(), Uuid::new_v4(),
        ).unwrap();
        assert!(outsider.trusted_output_path(output.to_str().unwrap()).unwrap().is_none());
    }

    #[test]
    fn tail_defaults_to_eight_megabytes_and_reports_omitted_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("tail.stdout");
        fs::write(&output, "prefix-hello-world".as_bytes()).unwrap();
        assert_eq!(tail_with_limit(&output, 100).unwrap(), "prefix-hello-world");
        let shortened = tail_with_limit(&output, 5).unwrap();
        assert_eq!(shortened, "[0KB of earlier output omitted]\nworld");
        assert_eq!(DISPLAY_CAP, 8 * 1024 * 1024);
    }

    #[tokio::test]
    async fn disk_cap_emits_single_original_notice_but_drains_remaining_output() {
        use tokio::io::AsyncWriteExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cap.stdout");
        let file = create_output_file(&path).unwrap();
        let (mut tx, rx) = tokio::io::duplex(32);
        let sending = tokio::spawn(async move {
            tx.write_all(b"0123456789abcdefghijklmnopqrstuvwxyz")
                .await
                .unwrap();
            tx.shutdown().await.unwrap();
        });
        assert!(spool_with_limit(rx, file, 10).await.unwrap());
        sending.await.unwrap();
        assert_eq!(
            fs::read_to_string(path).unwrap(),
            "0123456789\n[output truncated: exceeded 5GB disk cap]\n"
        );
    }

    #[test]
    fn delta_reads_incrementally_with_utf8_byte_offsets_and_no_symlink_follow() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let tasks = BackgroundTasks::new(&root).unwrap();
        let id = Uuid::new_v4().to_string();
        let path = tasks.directory.join(format!("{id}.stdout"));
        fs::write(&path, "héllo中文".as_bytes()).unwrap();
        let first: Value = serde_json::from_str(&tasks.output_delta(&id, 0, 3).unwrap()).unwrap();
        assert_eq!(first["bytesRead"], 3);
        assert_eq!(first["newOffset"], 3);
        let second: Value =
            serde_json::from_str(&tasks.output_delta(&id, 3, 800).unwrap()).unwrap();
        assert_eq!(second["content"], "llo中文");
        assert_eq!(second["newOffset"], "héllo中文".len());
        let zero: Value = serde_json::from_str(&tasks.output_delta(&id, 0, 0).unwrap()).unwrap();
        assert_eq!(zero["bytesRead"], 0);
        assert_eq!(zero["newOffset"], 0);
        let idle: Value = serde_json::from_str(&tasks.output_delta(&id, 999, 40).unwrap()).unwrap();
        assert_eq!(idle["newOffset"], 999);
        assert_eq!(idle["bytesRead"], 0);
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let protected = dir.path().join("protected");
            fs::write(&protected, "secret").unwrap();
            let _ = fs::remove_file(&path);
            symlink(&protected, &path).unwrap();
            assert!(tasks.output_delta(&id, 0, 10).is_err());
        }
    }

    #[cfg(unix)]
    #[test]
    fn task_output_never_follows_symlink_for_read_or_creation() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        let secret = temp.path().join("private");
        fs::write(&secret, "host private data").unwrap();
        let link = temp.path().join("output");
        symlink(&secret, &link).unwrap();
        assert!(create_output_file(&link).is_err());
        assert!(tail(link.to_str().unwrap()).is_err());
        assert_eq!(fs::read_to_string(&secret).unwrap(), "host private data");
    }

    #[tokio::test]
    async fn output_larger_than_previous_two_megabyte_cap_is_persisted() {
        use tokio::io::AsyncWriteExt;
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("large.stdout");
        let file = create_output_file(&path).unwrap();
        let (mut writer, reader) = tokio::io::duplex(64 * 1024);
        let send = tokio::spawn(async move {
            writer.write_all(&vec![b'x'; 2_200_000]).await.unwrap();
            writer.shutdown().await.unwrap();
        });
        assert!(!spool(reader, file).await.unwrap());
        send.await.unwrap();
        assert_eq!(fs::metadata(path).unwrap().len(), 2_200_000);
        assert_eq!(LOG_CAP, 5 * 1024 * 1024 * 1024);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn explicit_shell_override_does_not_mutate_background_default() {
        let dir = tempfile::tempdir().unwrap();
        let tasks = BackgroundTasks::new(&dir.path().canonicalize().unwrap()).unwrap();
        tasks.set_shell(crate::shell_runtime::ShellKind::Powershell);
        let started: Value = serde_json::from_str(
            &tasks
                .start_with_shell(
                    "printf 'isolated-shell'",
                    "Explicit Bash despite PowerShell default",
                    crate::shell_runtime::ShellKind::Bash,
                )
                .await
                .unwrap(),
        )
        .unwrap();
        let id = started["task_id"].as_str().unwrap();
        let output: Value =
            serde_json::from_str(&tasks.output(id, true, 5000).await.unwrap()).unwrap();
        assert!(output["task"]["output"]
            .as_str()
            .unwrap()
            .contains("isolated-shell"));
        assert_eq!(
            *tasks.shell_kind.lock().unwrap(),
            crate::shell_runtime::ShellKind::Powershell
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn combined_shell_log_preserves_stdout_and_stderr_order() {
        let dir = tempfile::tempdir().unwrap();
        let tasks = BackgroundTasks::new(&dir.path().canonicalize().unwrap()).unwrap();
        let created: Value = serde_json::from_str(&tasks.start_with_shell(
            "printf 'first\\n'; sleep 0.1; printf 'second\\n' >&2; sleep 0.1; printf 'third\\n'",
            "Combined shell output", crate::shell_runtime::ShellKind::Bash,
        ).await.unwrap()).unwrap();
        let id = created["task_id"].as_str().unwrap();
        let output: Value = serde_json::from_str(&tasks.output(id, true, 5000).await.unwrap()).unwrap();
        assert_eq!(output["task"]["output"], "first\nsecond\nthird\n");
        assert!(created["output_file"].as_str().unwrap().ends_with(".output"));
        let delta: Value = serde_json::from_str(&tasks.output_delta(id, 0, 100).unwrap()).unwrap();
        assert_eq!(delta["content"], "first\nsecond\nthird\n");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn completes_and_reads_output() {
        let dir = tempfile::tempdir().unwrap();
        let tasks = BackgroundTasks::new(&dir.path().canonicalize().unwrap()).unwrap();
        let created: Value = serde_json::from_str(
            &tasks
                .start("printf 'alpha\\n'", "Print output")
                .await
                .unwrap(),
        )
        .unwrap();
        let id = created["task_id"].as_str().unwrap();
        let output: Value =
            serde_json::from_str(&tasks.output(id, true, 5000).await.unwrap()).unwrap();
        assert_eq!(output["retrieval_status"], "success");
        assert_eq!(output["task"]["status"], "completed");
        assert!(output["task"]["output"].as_str().unwrap().contains("alpha"));
        assert_eq!(output["task"]["exitCode"], 0);
        let listing: Value = serde_json::from_str(&tasks.list().unwrap()).unwrap();
        assert_eq!(listing.as_array().unwrap().len(), 1);
        assert_eq!(listing[0]["status"], "completed");
        assert!(created["output_file"]
            .as_str()
            .is_some_and(|p| Path::new(p).exists()));
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn stops_and_recovers_completed_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let tasks = BackgroundTasks::new(&root).unwrap();
        let created: Value =
            serde_json::from_str(&tasks.start("sleep 30", "Long job").await.unwrap()).unwrap();
        let id = created["task_id"].as_str().unwrap();
        let pending: Value =
            serde_json::from_str(&tasks.output(id, false, 0).await.unwrap()).unwrap();
        assert_eq!(pending["retrieval_status"], "not_ready");
        assert!(tasks
            .stop(id)
            .await
            .unwrap()
            .contains("Successfully stopped task"));
        let result: Value =
            serde_json::from_str(&tasks.output(id, true, 5000).await.unwrap()).unwrap();
        assert_eq!(result["task"]["status"], "killed");
        let restarted = BackgroundTasks::new(&root).unwrap();
        let persisted: Value =
            serde_json::from_str(&restarted.output(id, false, 0).await.unwrap()).unwrap();
        assert_eq!(persisted["task"]["status"], "killed");
    }
    #[test]
    fn persisted_metadata_cannot_redirect_output_to_arbitrary_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let tasks = BackgroundTasks::new(&root).unwrap();
        let id = Uuid::new_v4().to_string();
        let safe_out = tasks.directory.join(format!("{id}.stdout"));
        let safe_err = tasks.directory.join(format!("{id}.stderr"));
        fs::write(&safe_out, "safe output").unwrap();
        fs::write(&safe_err, "").unwrap();
        let private = root.join("private.txt");
        fs::write(&private, "DO_NOT_DISCLOSE").unwrap();
        let forged = TaskRecord {
            id: id.clone(),
            description: "Forged record".into(),
            command: "nothing".into(),
            status: "completed".into(),
            exit_code: Some(0),
            output_file: private.to_string_lossy().to_string(),
            error_file: safe_err.to_string_lossy().to_string(),
            truncated: false,
        };
        BackgroundTasks::save(&tasks.directory, &forged).unwrap();
        let output: Value = serde_json::from_str(&tokio_test_output(&tasks, &id)).unwrap();
        assert!(output["task"]["output"]
            .as_str()
            .unwrap()
            .contains("safe output"));
        assert!(!output.to_string().contains("DO_NOT_DISCLOSE"));
    }

    #[test]
    fn recovers_unfinished_record_as_interrupted() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let tasks = BackgroundTasks::new(&root).unwrap();
        let id = Uuid::new_v4().to_string();
        let output = tasks.directory.join(format!("{id}.stdout"));
        let error = tasks.directory.join(format!("{id}.stderr"));
        fs::write(&output, "old result").unwrap();
        fs::write(&error, "").unwrap();
        let record = TaskRecord {
            id: id.clone(),
            description: "Orphan".into(),
            command: "old".into(),
            status: "running".into(),
            exit_code: None,
            output_file: output.to_string_lossy().to_string(),
            error_file: error.to_string_lossy().to_string(),
            truncated: false,
        };
        BackgroundTasks::save(&tasks.directory, &record).unwrap();
        let value: Value = serde_json::from_str(&tokio_test_output(&tasks, &id)).unwrap();
        assert_eq!(value["task"]["status"], "interrupted");
    }
    fn tokio_test_output(tasks: &BackgroundTasks, id: &str) -> String {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(tasks.output(id, false, 0))
            .unwrap()
    }
}
