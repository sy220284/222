use crate::protocol::ToolCall;
use anyhow::{bail, Context, Result};
use regex::Regex;
use serde_json::{json, Value};
use std::{
    fs,
    io::{self, Write},
    path::{Component, Path, PathBuf},
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
}

impl ToolExecutor {
    pub fn new(root: PathBuf, approval: Approval, interactive: bool) -> Self {
        Self { root, approval, interactive }
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
        match call.function.name.as_str() {
            "read_file" => self.read_file(&args),
            "list_files" => self.list_files(&args),
            "search_text" => self.search_text(&args),
            "write_file" => {
                self.require_approval("write_file", &args)?;
                self.write_file(&args).await
            }
            "edit_file" => {
                self.require_approval("edit_file", &args)?;
                self.edit_file(&args).await
            }
            "run_command" => {
                self.require_approval("run_command", &args)?;
                self.run_command(&args).await
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
        let start = args.get("start_line").and_then(Value::as_u64).unwrap_or(1).max(1) as usize;
        let count = args.get("line_count").and_then(Value::as_u64).unwrap_or(250).clamp(1, 500) as usize;
        let result = content.lines().enumerate()
            .skip(start - 1).take(count)
            .map(|(i, line)| format!("{}: {}", i + 1, line))
            .collect::<Vec<_>>().join("\n");
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
        proc.current_dir(&self.root).kill_on_drop(true);
        let output = timeout(Duration::from_secs(secs), proc.output())
            .await.context("Command timed out")??;
        Ok(format!(
            "Exit: {}\nSTDOUT:\n{}\nSTDERR:\n{}",
            output.status,
            truncate(&String::from_utf8_lossy(&output.stdout), 12_000),
            truncate(&String::from_utf8_lossy(&output.stderr), 4_000)
        ))
    }
}

fn required<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args.get(key).and_then(Value::as_str)
        .with_context(|| format!("Missing string argument: {key}"))
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
    vec![
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
                "command":{"type":"string"},"timeout_seconds":{"type":"integer"}
            },"required":["command"]}}}),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::FunctionCall;

    fn executor(root: &Path, approval: Approval) -> ToolExecutor {
        ToolExecutor::new(root.canonicalize().unwrap(), approval, false)
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
