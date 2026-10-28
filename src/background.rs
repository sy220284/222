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
use crate::{task_guards, task_output_format};

const LOG_CAP: usize = 2_000_000;
const DISPLAY_CAP: usize = 640_000;

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
    directory: PathBuf,
    entries: Mutex<HashMap<String, Arc<Running>>>,
}

impl BackgroundTasks {
    pub fn new(root: &Path) -> Result<Self> {
        let directory = root.join(".atlas").join("background");
        fs::create_dir_all(&directory)?;
        if !directory.canonicalize()?.starts_with(root) {
            bail!("Background task directory escapes workspace");
        }
        Ok(Self { directory, entries: Mutex::new(HashMap::new()) })
    }

    fn metadata_path(&self, id: &str) -> Result<PathBuf> {
        let uuid = Uuid::parse_str(id).context("Invalid task ID")?;
        Ok(self.directory.join(format!("{uuid}.json")))
    }

    fn save(directory: &Path, record: &TaskRecord) -> Result<()> {
        let path = directory.join(format!("{}.json", record.id));
        let temp = directory.join(format!("{}.tmp", record.id));
        fs::write(&temp, serde_json::to_vec_pretty(record)?)?;
        fs::rename(temp, path)?;
        Ok(())
    }

    fn load(&self, id: &str) -> Result<TaskRecord> {
        let path = self.metadata_path(id)?;
        let mut record: TaskRecord = serde_json::from_slice(&fs::read(path)?)?;
        if record.id != id { bail!("Task metadata ID mismatch"); }
        // Spawned processes are bound to their owning agent process.
        // A stale 'running' record must not masquerade as a live task.
        if record.status == "running" {
            record.status = "interrupted".to_owned();
            Self::save(&self.directory, &record)?;
        }
        Ok(record)
    }

    pub async fn start(&self, command: &str, description: &str) -> Result<String> {
        if command.trim().is_empty() { bail!("Background shell command is empty") }
        if command.len() > 100_000 { bail!("Background shell command too large") }
        let id = Uuid::new_v4().to_string();
        let stdout_path = self.directory.join(format!("{id}.stdout"));
        let stderr_path = self.directory.join(format!("{id}.stderr"));
        tokio::fs::write(&stdout_path, &[]).await?;
        tokio::fs::write(&stderr_path, &[]).await?;
        #[cfg(windows)]
        let mut proc = {
            let mut p = Command::new("cmd");
            p.arg("/C").arg(command);
            p
        };
        #[cfg(not(windows))]
        let mut proc = {
            let mut p = Command::new("sh");
            p.arg("-c").arg(command);
            p
        };
        let workspace = self.directory.parent().and_then(Path::parent)
            .context("Background task workspace missing")?;
        // Place the shell and its descendants into a new process group.
        // Stopping only the shell can otherwise leave children holding pipes.
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(proc.as_std_mut(), 0);
        proc.current_dir(workspace).stdin(Stdio::null())
            .stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
        let mut child = proc.spawn().context("Cannot start background command")?;
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
        Self::save(&self.directory, &record)?;
        let (stop, stopped) = oneshot::channel();
        let running = Arc::new(Running {
            record: Mutex::new(record.clone()),
            stop: Mutex::new(Some(stop)),
            completed: Notify::new(),
        });
        self.entries.lock().unwrap().insert(id.clone(), running.clone());
        let folder = self.directory.clone();
        tokio::spawn(async move {
            let stdout_task = tokio::spawn(spool(stdout, stdout_path));
            let stderr_task = tokio::spawn(spool(stderr, stderr_path));
            let (state, code) = tokio::select! {
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
            let out = stdout_task.await.ok().and_then(Result::ok).unwrap_or(true);
            let err = stderr_task.await.ok().and_then(Result::ok).unwrap_or(true);
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
        Ok(json!({"task_id":id,"task_type":"local_bash","status":"running",
            "output_file":record.output_file,"error_file":record.error_file}).to_string())
    }

    pub async fn output(&self, id: &str, block: bool, timeout_ms: u64) -> Result<String> {
        let running = self.entries.lock().unwrap().get(id).cloned();
        let record = if let Some(task) = running {
            let wait = async {
                loop {
                    let notified = task.completed.notified();
                    let current = task.record.lock().unwrap().clone();
                    if current.status != "running" { return current }
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
        let task_uuid = Uuid::parse_str(id).context("Invalid task ID")?;
        let safe_stdout = self.directory.join(format!("{task_uuid}.stdout"));
        let safe_stderr = self.directory.join(format!("{task_uuid}.stderr"));
        let stdout = tail(safe_stdout.to_str().context("Invalid stdout path")?)?;
        let stderr = tail(safe_stderr.to_str().context("Invalid stderr path")?)?;
        let text = [stdout, stderr].into_iter()
            .filter(|x| !x.is_empty()).collect::<Vec<_>>().join("\n");
        let displayed = task_output_format::format_task_output(
            &text, safe_stdout.to_str().context("Invalid task output path")?,
            task_output_format::max_output_length());
        let retrieval = if record.status == "running" {
            if block { "timeout" } else { "not_ready" }
        } else { "success" };
        debug_assert!(task_guards::is_local_shell_task(&json!({"type":"local_bash"})));
        Ok(json!({"retrieval_status":retrieval,"task":{
            "task_id":record.id,"task_type":"local_bash",
            "status":record.status,"description":record.description,
            "output":displayed.content,"exitCode":record.exit_code,
            "output_file":safe_stdout.to_string_lossy(),
            "truncated":record.truncated || displayed.was_truncated
        }}).to_string())
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
            if Uuid::parse_str(id).is_err() { continue }
            let record = if let Some(running) = live.get(id) {
                running.record.lock().unwrap().clone()
            } else {
                match self.load(id) { Ok(r) => r, Err(_) => continue }
            };
            records.push(json!({
                "task_id":record.id,
                "task_type":"local_bash",
                "description":record.description,
                "status":record.status,
                "exitCode":record.exit_code
            }));
        }
        records.sort_by(|a,b| a["task_id"].as_str().cmp(&b["task_id"].as_str()));
        Ok(serde_json::to_string_pretty(&records)?)
    }

    pub async fn stop(&self, id: &str) -> Result<String> {
        let task = self.entries.lock().unwrap().get(id).cloned()
            .with_context(|| format!("No running task with ID {id}"))?;
        if task.record.lock().unwrap().status != "running" {
            bail!("Task {id} is not running");
        }
        let sender = task.stop.lock().unwrap().take()
            .context("Task stop already requested")?;
        sender.send(()).map_err(|_| anyhow::anyhow!("Task already completed"))?;
        Ok(json!({"task_id":id,"task_type":"local_bash",
            "message":"Stop requested"}).to_string())
    }

    pub async fn stop_all(&self) {
        let tasks: Vec<_> = self.entries.lock().unwrap().values().cloned().collect();
        for task in tasks {
            if task.record.lock().unwrap().status == "running" {
                if let Some(sender) = task.stop.lock().unwrap().take() {
                    let _ = sender.send(());
                }
                let _ = timeout(Duration::from_secs(3), async {
                    loop {
                        let notified = task.completed.notified();
                        if task.record.lock().unwrap().status != "running" { break }
                        notified.await;
                    }
                }).await;
            }
        }
    }
}

fn tail(path: &str) -> Result<String> {
    let file = fs::File::open(path)?;
    let len = file.metadata()?.len();
    let start = len.saturating_sub(DISPLAY_CAP as u64);
    use std::io::{Read, Seek, SeekFrom};
    let mut file = file;
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    file.take(DISPLAY_CAP as u64).read_to_end(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

async fn spool<R: AsyncRead + Unpin>(mut stream: R, path: PathBuf) -> Result<bool> {
    let mut file = tokio::fs::File::create(path).await?;
    let mut buffer = [0u8; 8192];
    let mut size = 0usize;
    let mut truncated = false;
    loop {
        let n = stream.read(&mut buffer).await?;
        if n == 0 { break }
        let writable = n.min(LOG_CAP.saturating_sub(size));
        if writable > 0 {
            file.write_all(&buffer[..writable]).await?;
            size += writable;
        }
        if writable < n { truncated = true; }
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
        json!({"type":"function","function":{"name":"task_stop",
            "description":"Stop a running background shell task by ID. Requires approval.",
            "parameters":{"type":"object","properties":{
                "task_id":{"type":"string"}
            },"required":["task_id"]}}}),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    #[tokio::test]
    async fn completes_and_reads_output() {
        let dir = tempfile::tempdir().unwrap();
        let tasks = BackgroundTasks::new(&dir.path().canonicalize().unwrap()).unwrap();
        let created: Value = serde_json::from_str(
            &tasks.start("printf 'alpha\\n'", "Print output").await.unwrap()).unwrap();
        let id = created["task_id"].as_str().unwrap();
        let output: Value = serde_json::from_str(
            &tasks.output(id, true, 5000).await.unwrap()).unwrap();
        assert_eq!(output["retrieval_status"], "success");
        assert_eq!(output["task"]["status"], "completed");
        assert!(output["task"]["output"].as_str().unwrap().contains("alpha"));
        assert_eq!(output["task"]["exitCode"], 0);
        let listing: Value = serde_json::from_str(&tasks.list().unwrap()).unwrap();
        assert_eq!(listing.as_array().unwrap().len(), 1);
        assert_eq!(listing[0]["status"], "completed");
        assert!(created["output_file"].as_str().is_some_and(|p| Path::new(p).exists()));
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn stops_and_recovers_completed_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let tasks = BackgroundTasks::new(&root).unwrap();
        let created: Value = serde_json::from_str(
            &tasks.start("sleep 30", "Long job").await.unwrap()).unwrap();
        let id = created["task_id"].as_str().unwrap();
        let pending: Value = serde_json::from_str(
            &tasks.output(id, false, 0).await.unwrap()).unwrap();
        assert_eq!(pending["retrieval_status"], "not_ready");
        assert!(tasks.stop(id).await.unwrap().contains("Stop requested"));
        let result: Value = serde_json::from_str(
            &tasks.output(id, true, 5000).await.unwrap()).unwrap();
        assert_eq!(result["task"]["status"], "killed");
        let restarted = BackgroundTasks::new(&root).unwrap();
        let persisted: Value = serde_json::from_str(
            &restarted.output(id, false, 0).await.unwrap()).unwrap();
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
            id: id.clone(), description:"Forged record".into(), command:"nothing".into(),
            status:"completed".into(), exit_code:Some(0),
            output_file:private.to_string_lossy().to_string(),
            error_file:safe_err.to_string_lossy().to_string(),truncated:false
        };
        BackgroundTasks::save(&tasks.directory,&forged).unwrap();
        let output: Value = serde_json::from_str(&tokio_test_output(&tasks,&id)).unwrap();
        assert!(output["task"]["output"].as_str().unwrap().contains("safe output"));
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
            id: id.clone(), description: "Orphan".into(), command: "old".into(),
            status: "running".into(), exit_code: None,
            output_file: output.to_string_lossy().to_string(),
            error_file: error.to_string_lossy().to_string(),
            truncated: false,
        };
        BackgroundTasks::save(&tasks.directory, &record).unwrap();
        let value: Value = serde_json::from_str(
            &tokio_test_output(&tasks, &id)).unwrap();
        assert_eq!(value["task"]["status"], "interrupted");
    }
    fn tokio_test_output(tasks: &BackgroundTasks, id: &str) -> String {
        tokio::runtime::Builder::new_current_thread().enable_all()
            .build().unwrap().block_on(tasks.output(id, false, 0)).unwrap()
    }
}
