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
    sync::{oneshot, Notify},
    time::timeout,
};
use uuid::Uuid;

// Original diskOutput.ts allows up to 5 GiB of task output per stream.
// Keep the cap in bytes (u64), not characters, to preserve large outputs.
const LOG_CAP: u64 = 5 * 1024 * 1024 * 1024;
const DEFAULT_DELTA_READ_BYTES: u64 = 8 * 1024 * 1024;
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
    completed: Notify,
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
        Self::in_directory(
            root,
            root.join(".atlas")
                .join("background")
                .join(session_id.to_string()),
        )
    }

    fn in_directory(root: &Path, directory: PathBuf) -> Result<Self> {
        fs::create_dir_all(&directory)?;
        if !directory.canonicalize()?.starts_with(root) {
            bail!("Background task directory escapes workspace");
        }
        Ok(Self {
            workspace: root.to_path_buf(),
            directory,
            entries: Mutex::new(HashMap::new()),
            shell_kind: Mutex::new(crate::shell_runtime::ShellKind::default()),
            session_env: Mutex::new(std::collections::BTreeMap::new()),
        })
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
            })
            .context("Could not allocate a unique background task ID")?;
        // Validate the shell command before reserving output files: a
        // command-construction failure must not leave stale task artifacts.
        let mut proc = kind.build_command(command)?;
        proc.envs(self.session_env.lock().unwrap().iter());
        crate::subprocess_env::apply(&mut proc);
        let stdout_path = self.directory.join(format!("{id}.stdout"));
        let stderr_path = self.directory.join(format!("{id}.stderr"));
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
                let _ = fs::remove_file(&stdout_path);
                let _ = fs::remove_file(&stderr_path);
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
            output_file: stdout_path.to_string_lossy().to_string(),
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
            let _ = fs::remove_file(&stdout_path);
            let _ = fs::remove_file(&stderr_path);
            return Err(error).context("Persist background task registration");
        }
        let (stop, stopped) = oneshot::channel();
        let running = Arc::new(Running {
            record: Mutex::new(record.clone()),
            stop: Mutex::new(Some(stop)),
            completed: Notify::new(),
        });
        self.entries
            .lock()
            .unwrap()
            .insert(id.clone(), running.clone());
        let folder = self.directory.clone();
        tokio::spawn(async move {
            let stdout_task = tokio::spawn(spool(stdout, stdout_file));
            let stderr_task = tokio::spawn(spool(stderr, stderr_file));
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
            if stdout_failed || stderr_failed {
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
            let _ = Self::save(&folder, &updated);
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
        let stdout = tail(safe_stdout.to_str().context("Invalid stdout path")?)?;
        let stderr = tail(safe_stderr.to_str().context("Invalid stderr path")?)?;
        let text = [stdout, stderr]
            .into_iter()
            .filter(|x| !x.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        let displayed = task_output_format::format_task_output(
            &text,
            safe_stdout.to_str().context("Invalid task output path")?,
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
            "output_file":safe_stdout.to_string_lossy(),
            "truncated":record.truncated || displayed.was_truncated
        }})
        .to_string())
    }

    /// Read only newly appended stdout bytes, using the original task
    /// framework's offset/delta contract. A stopped task's file remains
    /// readable across agent restarts. Never trust persisted path metadata.
    pub fn output_delta(&self, id: &str, from_offset: u64, max_bytes: u64) -> Result<String> {
        let id = canonical_shell_task_id(id)?;
        let path = self.directory.join(format!("{id}.stdout"));
        // Explicit zero is a zero-byte read, not an implicit default.
        let max_bytes = max_bytes.min(DEFAULT_DELTA_READ_BYTES);
        let delta = read_output_range(&path, from_offset, max_bytes)?;
        let (content, bytes_read, total) = match delta {
            Some((content, bytes_read, total)) => (content, bytes_read, total),
            None => (String::new(), 0, from_offset),
        };
        Ok(json!({
            "task_id":id,
            "content":content,
            "newOffset":from_offset.saturating_add(bytes_read),
            "bytesRead":bytes_read,
            "bytesTotal":total,
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
                "exitCode":record.exit_code
            }));
        }
        records.sort_by(|a, b| a["task_id"].as_str().cmp(&b["task_id"].as_str()));
        Ok(serde_json::to_string_pretty(&records)?)
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
    let mut bytes = Vec::with_capacity((total - offset).min(max_bytes) as usize);
    file.take((total - offset).min(max_bytes))
        .read_to_end(&mut bytes)?;
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
            "description":"Read appended stdout by byte offset; max 8MiB per read; never follows a symlink.",
            "parameters":{"type":"object","properties":{
                "task_id":{"type":"string"},"from_offset":{"type":"integer","minimum":0},
                "max_bytes":{"type":"integer","minimum":1,"maximum":8388608}
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
