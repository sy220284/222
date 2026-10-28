use crate::{protocol::ToolCall, notebook, search, webfetch, history, plugins, websearch, config::SearchBackend, background::BackgroundTasks};
use anyhow::{bail, Context, Result};
use regex::Regex;
use serde_json::{json, Value};
use std::{
    fs,
    io::{self, Write},
    path::{Component, Path, PathBuf},
    process::Stdio,
    sync::{Mutex, Arc},
    collections::HashMap,
    time::Duration,
};
use tokio::{process::Command, time::timeout};
use walkdir::WalkDir;

const MAX_FILE_BYTES: u64 = 1_048_576;
const MAX_OUTPUT_CHARS: usize = 16_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Approval {
    ReadOnly,
    Ask,
    Full,
}

pub struct ToolExecutor {
    root: PathBuf,
    approval: Approval,
    interactive: bool,
    reads: Mutex<HashMap<PathBuf, u64>>,
    enabled_plugins: Vec<String>,
    search_backend: Option<SearchBackend>,
    background: Option<Arc<BackgroundTasks>>,
}

impl ToolExecutor {
    pub fn new(root: PathBuf, approval: Approval, interactive: bool) -> Self {
        Self { root, approval, interactive, reads: Mutex::new(HashMap::new()), enabled_plugins: Vec::new(), search_backend: None, background: None }
    }

    pub fn with_background(mut self, background: Arc<BackgroundTasks>) -> Self {
        self.background = Some(background);
        self
    }

    pub fn with_search(mut self,backend:Option<SearchBackend>)->Self{
        self.search_backend=backend;
        self
    }

    pub fn with_plugins(mut self,enabled:Vec<String>)->Self{
        self.enabled_plugins=enabled;
        self
    }

    pub async fn execute(&self, call: &ToolCall) -> String {
        match self.dispatch(call).await {
            Ok(output) => truncate(&output, MAX_OUTPUT_CHARS),
            Err(err) => format!("ERROR: {err:#}"),
        }
    }

    async fn dispatch(&self, call: &ToolCall) -> Result<String> {
        let args: Value = serde_json::from_str(&call.function.arguments)
            .with_context(|| format!("Invalid JSON arguments for {}", call.function.name))?;
        if !args.is_object() { bail!("Tool arguments must be a JSON object") }
        match crate::tool_names::canonical_name(&call.function.name) {
            "read_file" => self.read_file(&args),
            "notebook_edit" => {
                self.require_approval("notebook_edit", &args)?;
                let path=self.checked_path(required(&args,"notebook_path")?)?;
                let previous=history::snapshot(&path)?;
                let result=self.notebook_edit(&args)?;
                let checkpoint=history::record(&self.root,&path,previous)?;
                Ok(format!("{result}{}", checkpoint.map(|id|format!("\nCheckpoint: {id}")).unwrap_or_default()))
            },
            "list_files" => self.list_files(&args),
            "glob_files" => {
                let path = self.checked_path(args.get("path").and_then(Value::as_str).unwrap_or("."))?;
                search::glob_files(&self.root, &path, &args)
            },
            "grep_files" => {
                let path = self.checked_path(args.get("path").and_then(Value::as_str).unwrap_or("."))?;
                search::grep_files(&self.root, &path, &args)
            },
            "search_text" => self.search_text(&args),
            "list_skills" => self.list_skills(),
            "read_skill" => self.read_skill(&args),
            "web_search" => {
                self.require_approval("web_search",&args)?;
                let backend=self.search_backend.as_ref().context("No search backend configured")?;
                websearch::execute(backend,&args).await
            },
            "web_fetch" => {
                self.require_approval("web_fetch", &args)?;
                webfetch::execute(&args).await
            },
            "git_status" => self.git_read(vec!["status".into(), "--short".into()]).await,
            "git_diff" => self.git_diff(&args).await,
            "git_log" => self.git_log(&args).await,
            "write_file" => {
                self.require_approval("write_file", &args)?;
                let path=self.checked_path(required(&args,"path")?)?;
                let before=history::snapshot(&path)?;
                let result=self.write_file(&args).await?;
                let checkpoint=history::record(&self.root,&path,before)?;
                Ok(format!("{result}{}",checkpoint.map(|id|format!("\nCheckpoint: {id}")).unwrap_or_default()))
            }
            "edit_file" => {
                self.require_approval("edit_file", &args)?;
                let path=self.checked_path(required(&args,"path")?)?;
                let before=history::snapshot(&path)?;
                let result=self.edit_file(&args).await?;
                let checkpoint=history::record(&self.root,&path,before)?;
                Ok(format!("{result}{}",checkpoint.map(|id|format!("\nCheckpoint: {id}")).unwrap_or_default()))
            }
            "file_history" => history::list(&self.root),
            "file_revert" => {
                self.require_approval("file_revert", &args)?;
                let record=history::load(&self.root,required(&args,"id")?)?;
                let path=self.checked_path(&record.path)?;
                history::restore(&record,&path)
            },
            "run_command" => {
                self.require_approval("run_command", &args)?;
                if args.get("run_in_background").and_then(Value::as_bool).unwrap_or(false) {
                    let manager = self.background.as_ref().context("Background tasks unavailable")?;
                    manager.start(required(&args,"command")?,
                        args.get("description").and_then(Value::as_str).unwrap_or("Shell command")).await
                } else {
                    self.run_command(&args).await
                }
            }
            "background_list" => {
                let manager = self.background.as_ref().context("Background tasks unavailable")?;
                manager.list()
            }
            "task_output" => {
                let manager = self.background.as_ref().context("Background tasks unavailable")?;
                manager.output(required(&args,"task_id")?,
                    args.get("block").and_then(Value::as_bool).unwrap_or(true),
                    args.get("timeout").and_then(Value::as_u64).unwrap_or(30_000)).await
            }
            "task_stop" | "KillShell" => {
                self.require_approval("task_stop",&args)?;
                let manager = self.background.as_ref().context("Background tasks unavailable")?;
                let id = args.get("task_id").and_then(Value::as_str)
                    .or_else(||args.get("shell_id").and_then(Value::as_str))
                    .context("Missing required parameter: task_id")?;
                manager.stop(id).await
            }
            other => bail!("Unknown tool: {other}"),
        }
    }

    pub fn approve_external(&self, call: &ToolCall) -> Result<()> {
        let args: Value = serde_json::from_str(&call.function.arguments)?;
        self.require_approval(&call.function.name, &args)
    }

    fn require_approval(&self, tool: &str, args: &Value) -> Result<()> {
        match self.approval {
            Approval::Full => Ok(()),
            Approval::ReadOnly => bail!("Tool {tool} denied in read-only mode"),
            Approval::Ask if !self.interactive => bail!(
                "Tool {tool} needs approval; noninteractive mode requires --approval full"
            ),
            Approval::Ask => {
                eprintln!("\nProposed {tool}: {}", truncate(&args.to_string(), 600));
                eprint!("Allow once? [y/N] ");
                io::stderr().flush()?;
                let mut answer = String::new();
                io::stdin().read_line(&mut answer)?;
                if answer.trim().eq_ignore_ascii_case("y") ||
                   answer.trim().eq_ignore_ascii_case("yes") {
                    Ok(())
                } else {
                    bail!("User rejected {tool}")
                }
            }
        }
    }

    /// Resolve paths lexically and check the nearest real ancestor against root.
    /// Existing symlinks that lead outside the workspace are rejected.
    fn checked_path(&self, raw: &str) -> Result<PathBuf> {
        if raw.is_empty() { bail!("Empty file path") }
        let input = Path::new(raw);
        let path = if input.is_absolute() { input.to_path_buf() } else { self.root.join(input) };
        let mut normalized = PathBuf::new();
        for part in path.components() {
            match part {
                Component::Prefix(_) | Component::RootDir | Component::Normal(_) =>
                    normalized.push(part.as_os_str()),
                Component::CurDir => {}
                Component::ParentDir => {
                    if !normalized.pop() { bail!("Path escapes filesystem root") }
                }
            }
        }
        if !normalized.starts_with(&self.root) {
            bail!("Path outside workspace: {}", normalized.display());
        }
        let mut ancestor = normalized.as_path();
        while !ancestor.exists() {
            ancestor = ancestor.parent().context("No existing ancestor")?;
        }
        let real = ancestor.canonicalize()?;
        if !real.starts_with(&self.root) {
            bail!("Path traverses a symlink outside workspace");
        }
        Ok(normalized)
    }

    fn read_file(&self, args: &Value) -> Result<String> {
        let path = self.checked_path(required(args, "path")?)?;
        let metadata = fs::metadata(&path)?;
        if metadata.len() > MAX_FILE_BYTES {
            bail!("File too large (limit {MAX_FILE_BYTES} bytes)");
        }
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Reading {}", path.display()))?;
        self.reads.lock().unwrap().insert(path, notebook::revision(&content));
        let start = args.get("start_line").and_then(Value::as_u64).unwrap_or(1).max(1) as usize;
        let count = args.get("line_count").and_then(Value::as_u64).unwrap_or(250).clamp(1, 500) as usize;
        let result = content.lines().enumerate()
            .skip(start - 1).take(count)
            .map(|(i, line)| format!("{}: {}", i + 1, line))
            .collect::<Vec<_>>().join("\n");
        Ok(result)
    }

    fn notebook_edit(&self, args: &Value) -> Result<String> {
        let raw = required(args, "notebook_path")?;
        let path = self.checked_path(raw)?;
        let known = self.reads.lock().unwrap().get(&path).copied();
        let (result, revision) = notebook::edit(&path, args, known)?;
        self.reads.lock().unwrap().insert(path, revision);
        Ok(result)
    }

    fn list_files(&self, args: &Value) -> Result<String> {
        let rel = args.get("path").and_then(Value::as_str).unwrap_or(".");
        let path = self.checked_path(rel)?;
        let depth = args.get("max_depth").and_then(Value::as_u64).unwrap_or(5).clamp(1, 20) as usize;
        let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(200).clamp(1, 1000) as usize;
        let mut entries = Vec::new();
        for entry in WalkDir::new(&path).max_depth(depth).into_iter().filter_entry(in_scope) {
            let entry = entry?;
            if entry.path() == path { continue; }
            entries.push(entry.path().strip_prefix(&self.root)?.display().to_string());
            if entries.len() >= limit { break; }
        }
        Ok(entries.join("\n"))
    }

    fn search_text(&self, args: &Value) -> Result<String> {
        let pattern = required(args, "pattern")?;
        let regex = Regex::new(pattern)?;
        let path = self.checked_path(args.get("path").and_then(Value::as_str).unwrap_or("."))?;
        let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(80).clamp(1, 300) as usize;
        let mut matches = Vec::new();
        for entry in WalkDir::new(path).into_iter().filter_entry(in_scope) {
            let entry = entry?;
            if !entry.file_type().is_file() { continue; }
            if entry.metadata()?.len() > MAX_FILE_BYTES { continue; }
            let Ok(data) = fs::read_to_string(entry.path()) else { continue };
            for (line_no, line) in data.lines().enumerate() {
                if regex.is_match(line) {
                    matches.push(format!("{}:{}: {}", entry.path().strip_prefix(&self.root)?
                        .display(), line_no + 1, truncate(line, 300)));
                    if matches.len() >= limit { return Ok(matches.join("\n")); }
                }
            }
        }
        Ok(matches.join("\n"))
    }

    fn list_skills(&self) -> Result<String> {
        let root = self.root.join(".atlas").join("skills");
        let mut skills = Vec::new();
        if root.is_dir() {
            for entry in fs::read_dir(root)? {
                let entry = entry?;
                let name = entry.file_name().to_string_lossy().to_string();
                if !valid_skill_name(&name) { continue; }
                let path = self.checked_path(&format!(".atlas/skills/{name}/SKILL.md"))?;
                if path.is_file() { skills.push(name); }
            }
        }
        skills.extend(plugins::list_skills(&self.root,&self.enabled_plugins)?);
        skills.sort();
        Ok(skills.join("\n"))
    }

    fn read_skill(&self, args: &Value) -> Result<String> {
        let name = required(args, "name")?;
        if name.contains(':') { return plugins::read_skill(&self.root,&self.enabled_plugins,name); }
        if !valid_skill_name(name) { bail!("Invalid skill name") }
        let path = self.checked_path(&format!(".atlas/skills/{name}/SKILL.md"))?;
        if fs::metadata(&path)?.len() > 64_000 { bail!("Skill file too large") }
        Ok(fs::read_to_string(path)?)
    }

    async fn git_read(&self, arguments: Vec<String>) -> Result<String> {
        let mut command = Command::new("git");
        command.arg("-C").arg(&self.root).arg("--no-pager")
            .args(arguments).kill_on_drop(true);
        let output = timeout(Duration::from_secs(30), command.output())
            .await.context("Git command timed out")??;
        if !output.status.success() {
            bail!("Git error: {}", String::from_utf8_lossy(&output.stderr));
        }
        Ok(truncate(&String::from_utf8_lossy(&output.stdout), MAX_OUTPUT_CHARS))
    }

    async fn git_diff(&self, args: &Value) -> Result<String> {
        let mut command = vec![
            "diff".into(), "--no-ext-diff".into(), "--no-textconv".into(),
            "--no-color".into()
        ];
        if args.get("staged").and_then(Value::as_bool).unwrap_or(false) {
            command.push("--cached".into());
        }
        if let Some(path) = args.get("path").and_then(Value::as_str) {
            let checked = self.checked_path(path)?;
            let relative = checked.strip_prefix(&self.root)?;
            command.push("--".into());
            command.push(relative.to_string_lossy().to_string());
        }
        self.git_read(command).await
    }

    async fn git_log(&self, args: &Value) -> Result<String> {
        let count = args.get("count").and_then(Value::as_u64).unwrap_or(5).clamp(1, 30);
        self.git_read(vec![
            "log".into(), format!("-{count}"), "--oneline".into()
        ]).await
    }

    async fn write_file(&self, args: &Value) -> Result<String> {
        let path = self.checked_path(required(args, "path")?)?;
        let content = required(args, "content")?;
        if content.len() as u64 > MAX_FILE_BYTES { bail!("Content too large") }
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        if args.get("create_only").and_then(Value::as_bool).unwrap_or(false) {
            use tokio::io::AsyncWriteExt;
            let mut file = tokio::fs::OpenOptions::new().write(true).create_new(true).open(&path).await?;
            file.write_all(content.as_bytes()).await?;
        } else {
            tokio::fs::write(&path, content).await?;
        }
        Ok(format!("Wrote {} bytes to {}", content.len(), path.display()))
    }

    async fn edit_file(&self, args: &Value) -> Result<String> {
        let path = self.checked_path(required(args, "path")?)?;
        let old = required(args, "old")?;
        let new = required(args, "new")?;
        if old.is_empty() { bail!("Replacement target cannot be empty") }
        if fs::metadata(&path)?.len() > MAX_FILE_BYTES { bail!("File too large") }
        let content = tokio::fs::read_to_string(&path).await?;
        let hits = content.matches(old).count();
        if hits != 1 { bail!("Expected exactly one match; found {hits}") }
        let updated = content.replacen(old, new, 1);
        if updated.len() as u64 > MAX_FILE_BYTES { bail!("Updated file too large") }
        tokio::fs::write(&path, updated).await?;
        Ok(format!("Replaced one occurrence in {}", path.display()))
    }

    async fn run_command(&self, args: &Value) -> Result<String> {
        let command = required(args, "command")?;
        let secs = args.get("timeout_seconds").and_then(Value::as_u64)
            .unwrap_or(60).clamp(1, 300);
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
        proc.current_dir(&self.root).kill_on_drop(true)
            .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = proc.spawn()?;
        let stdout = child.stdout.take().context("No command stdout")?;
        let stderr = child.stderr.take().context("No command stderr")?;
        let (status, out, err) = timeout(Duration::from_secs(secs), async {
            let (out, err) = tokio::try_join!(
                capped_read(stdout, 128_000),
                capped_read(stderr, 32_000)
            )?;
            let status = child.wait().await?;
            Ok::<_, anyhow::Error>((status, out, err))
        }).await.context("Command timed out")??;
        Ok(format!(
            "Exit: {}\nSTDOUT:\n{}\nSTDERR:\n{}",
            status,
            truncate(&String::from_utf8_lossy(&out), 12_000),
            truncate(&String::from_utf8_lossy(&err), 4_000)
        ))
    }
}

async fn capped_read<R: tokio::io::AsyncRead + Unpin>(
    mut input: R, max_bytes: usize
) -> Result<Vec<u8>> {
    use tokio::io::AsyncReadExt;
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        let n = input.read(&mut buffer).await?;
        if n == 0 { break; }
        if bytes.len() + n > max_bytes {
            bail!("Command output exceeded {max_bytes} bytes");
        }
        bytes.extend_from_slice(&buffer[..n]);
    }
    Ok(bytes)
}

fn required<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args.get(key).and_then(Value::as_str)
        .with_context(|| format!("Missing string argument: {key}"))
}

fn valid_skill_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 64 &&
        name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn in_scope(entry: &walkdir::DirEntry) -> bool {
    if entry.depth() == 0 { return true }
    !matches!(entry.file_name().to_str(), Some(".git" | ".atlas" | "target" | "node_modules"))
}

fn truncate(input: &str, max: usize) -> String {
    let mut chars = input.chars();
    let head: String = chars.by_ref().take(max).collect();
    if chars.next().is_some() { format!("{head}\n...[truncated]") } else { head }
}

pub fn definitions() -> Vec<Value> {
    let mut tools = vec![
        json!({"type":"function","function":{"name":"read_file",
            "description":"Read numbered lines from a workspace text file.",
            "parameters":{"type":"object","properties":{
                "path":{"type":"string"},"start_line":{"type":"integer","minimum":1},
                "line_count":{"type":"integer","minimum":1,"maximum":500}
            },"required":["path"]}}}),
        json!({"type":"function","function":{"name":"list_files",
            "description":"List workspace paths, excluding dependency and internal directories.",
            "parameters":{"type":"object","properties":{
                "path":{"type":"string"},"max_depth":{"type":"integer"},
                "limit":{"type":"integer"}
            }}}}),
        json!({"type":"function","function":{"name":"search_text",
            "description":"Find regex matches in workspace text files, with paths and line numbers.",
            "parameters":{"type":"object","properties":{
                "pattern":{"type":"string"},"path":{"type":"string"},"limit":{"type":"integer"}
            },"required":["pattern"]}}}),
        json!({"type":"function","function":{"name":"git_status",
            "description":"View Git working tree status for the project.",
            "parameters":{"type":"object","properties":{}}}}),
        json!({"type":"function","function":{"name":"git_diff",
            "description":"Show unstaged or staged Git diff, optionally limited to a file.",
            "parameters":{"type":"object","properties":{
                "path":{"type":"string"},"staged":{"type":"boolean"}
            }}}}),
        json!({"type":"function","function":{"name":"git_log",
            "description":"Show recent Git commit summaries.",
            "parameters":{"type":"object","properties":{
                "count":{"type":"integer"}
            }}}}),
        json!({"type":"function","function":{"name":"list_skills",
            "description":"List available project skills in .atlas/skills.",
            "parameters":{"type":"object","properties":{}}}}),
        json!({"type":"function","function":{"name":"read_skill",
            "description":"Load the instructions of a named project skill.",
            "parameters":{"type":"object","properties":{"name":{"type":"string"}},"required":["name"]}}}),
        json!({"type":"function","function":{"name":"write_file",
            "description":"Create or overwrite a workspace text file. Requires approval.",
            "parameters":{"type":"object","properties":{
                "path":{"type":"string"},"content":{"type":"string"},
                "create_only":{"type":"boolean"}
            },"required":["path","content"]}}}),
        json!({"type":"function","function":{"name":"edit_file",
            "description":"Replace exactly one matching text span in a workspace file. Requires approval.",
            "parameters":{"type":"object","properties":{
                "path":{"type":"string"},"old":{"type":"string"},"new":{"type":"string"}
            },"required":["path","old","new"]}}}),
        json!({"type":"function","function":{"name":"run_command",
            "description":"Execute a shell command from the workspace. Requires explicit approval. Commands can access the entire host.",
            "parameters":{"type":"object","properties":{
                "command":{"type":"string"},"timeout_seconds":{"type":"integer"},
                "run_in_background":{"type":"boolean"},"description":{"type":"string"}
            },"required":["command"]}}}),
    ];
    tools.push(notebook::definition());
    tools.extend(search::definitions());
    tools.push(webfetch::definition());
    tools.extend(history::definitions());
    tools.extend(crate::background::definitions());
    tools
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::FunctionCall;

    fn executor(root: &Path, approval: Approval) -> ToolExecutor {
        ToolExecutor::new(root.canonicalize().unwrap(), approval, false)
    }


    #[cfg(unix)]
    #[tokio::test]
    async fn shell_command_respects_output_limit() {
        let root = tempfile::tempdir().unwrap();
        let executor = executor(root.path(), Approval::Full);
        let small = ToolCall {
            id: "ok".into(), kind: "function".into(),
            function: FunctionCall {
                name: "run_command".into(),
                arguments: json!({"command":"printf hello"}).to_string(),
            },
        };
        assert!(executor.execute(&small).await.contains("hello"));
        let large = ToolCall {
            id: "large".into(), kind: "function".into(),
            function: FunctionCall {
                name: "run_command".into(),
                arguments: json!({"command":"head -c 200000 /dev/zero"}).to_string(),
            },
        };
        assert!(executor.execute(&large).await.contains("Command output exceeded"));
    }

    #[tokio::test]
    async fn git_status_and_diff_read_real_repository() {
        let root = tempfile::tempdir().unwrap();
        let run = |args: &[&str]| {
            let output = std::process::Command::new("git")
                .current_dir(root.path()).args(args).output().unwrap();
            assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        };
        run(&["init", "-q"]);
        fs::write(root.path().join("hello.txt"), "before\n").unwrap();
        run(&["add", "hello.txt"]);
        run(&["-c", "user.name=Test", "-c", "user.email=test@example.com",
            "commit", "-qm", "initial"]);
        fs::write(root.path().join("hello.txt"), "after\n").unwrap();
        let tool = executor(root.path(), Approval::ReadOnly);
        let status = tool.git_read(vec!["status".into(), "--short".into()]).await.unwrap();
        assert!(status.contains("hello.txt"));
        let diff = tool.git_diff(&json!({"path":"hello.txt"})).await.unwrap();
        assert!(diff.contains("-before"));
        assert!(diff.contains("+after"));
        assert!(tool.git_log(&json!({"count":1})).await.unwrap().contains("initial"));
    }

    #[test]
    fn enabled_plugin_skills_are_found_without_project_skills_directory() {
        let root=tempfile::tempdir().unwrap();
        let p=root.path().join(".atlas/plugins/demo");
        fs::create_dir_all(p.join("skills/build")).unwrap();
        fs::write(p.join("skills/build/SKILL.md"),"Build instructions").unwrap();
        let tool=executor(root.path(),Approval::ReadOnly)
            .with_plugins(vec!["demo".into()]);
        assert_eq!(tool.list_skills().unwrap(),"demo:build");
        assert!(tool.read_skill(&json!({"name":"demo:build"}))
            .unwrap().contains("Build instructions"));
    }

    #[test]
    fn skills_are_discovered_and_loaded_from_workspace() {
        let root = tempfile::tempdir().unwrap();
        let skill = root.path().join(".atlas/skills/test_skill");
        fs::create_dir_all(&skill).unwrap();
        fs::write(skill.join("SKILL.md"), "# Useful guide\nDo the task.").unwrap();
        let executor = executor(root.path(), Approval::ReadOnly);
        assert_eq!(executor.list_skills().unwrap(), "test_skill");
        let content = executor.read_skill(&json!({"name":"test_skill"})).unwrap();
        assert!(content.contains("Useful guide"));
        assert!(executor.read_skill(&json!({"name":"../nope"})).is_err());
    }

    #[test]
    fn blocks_parent_traversal() {
        let root = tempfile::tempdir().unwrap();
        let tools = executor(root.path(), Approval::Full);
        assert!(tools.checked_path("../escape").is_err());
        assert!(tools.checked_path("sub/../../escape").is_err());
        assert!(tools.checked_path("sub/ok.txt").is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn blocks_symlink_escape() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("out")).unwrap();
        let tools = executor(root.path(), Approval::Full);
        assert!(tools.checked_path("out/secret").is_err());
    }

    #[tokio::test]
    async fn edit_requires_one_match() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("a.txt"), "x x").unwrap();
        let tools = executor(root.path(), Approval::Full);
        let call = ToolCall {
            id: "1".into(), kind: "function".into(),
            function: FunctionCall {
                name: "edit_file".into(),
                arguments: json!({"path":"a.txt","old":"x","new":"y"}).to_string(),
            },
        };
        assert!(tools.execute(&call).await.contains("Expected exactly one match"));
        assert_eq!(fs::read_to_string(root.path().join("a.txt")).unwrap(), "x x");
    }

    #[tokio::test]
    async fn noninteractive_requires_permission_for_writes() {
        let root = tempfile::tempdir().unwrap();
        let tools = executor(root.path(), Approval::Ask);
        let call = ToolCall {
            id: "1".into(), kind: "function".into(),
            function: FunctionCall {
                name: "write_file".into(),
                arguments: json!({"path":"a.txt","content":"hello"}).to_string(),
            },
        };
        assert!(tools.execute(&call).await.contains("needs approval"));
        assert!(!root.path().join("a.txt").exists());
    }
}
