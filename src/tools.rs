use crate::{
    background::BackgroundTasks, config::SearchBackend, history, notebook, permission_mode,
    permission_rules, plugins, protocol::ToolCall, search, tool_metrics::ToolMetrics, webfetch,
    websearch,
};
use anyhow::{bail, Context, Result};
use regex::Regex;
use serde_json::{json, Value};
use std::{
    collections::{HashMap, VecDeque},
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex},
    time::{Duration, UNIX_EPOCH},
};
use tokio::{process::Command, time::timeout};
use walkdir::WalkDir;

const MAX_FILE_BYTES: u64 = 1_048_576;
const MAX_OUTPUT_CHARS: usize = 16_000;

/// Stream task output one line at a time. A single unbroken line can be
/// gigabytes long, so skip and truncate it without allocating the whole file.
/// The full byte stream remains available through task_output_delta.
fn bounded_task_line<R: io::BufRead>(
    reader: &mut R,
    limit: usize,
) -> Result<Option<(Vec<u8>, bool)>> {
    let mut bytes = Vec::new();
    let mut seen = false;
    let mut truncated = false;
    loop {
        let chunk = reader.fill_buf()?;
        if chunk.is_empty() {
            break;
        }
        seen = true;
        let newline = chunk.iter().position(|byte| *byte == b'\n');
        let count = newline.map(|index| index + 1).unwrap_or(chunk.len());
        let append = count.min(limit.saturating_sub(bytes.len()));
        bytes.extend_from_slice(&chunk[..append]);
        if append < count {
            truncated = true;
        }
        reader.consume(count);
        if newline.is_some() {
            break;
        }
    }
    if !seen {
        return Ok(None);
    }
    Ok(Some((bytes, truncated)))
}

fn read_trusted_task_output(path: &Path, args: &Value, source_read: bool) -> Result<String> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        bail!("Task output must be a regular file");
    }
    let start = args
        .get("start_line")
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .max(1);
    let count = args
        .get("line_count")
        .and_then(Value::as_u64)
        .unwrap_or(if source_read { u64::MAX } else { 250 })
        .max(1);
    let count = if source_read { count } else { count.min(500) };
    let end = start.saturating_add(count);
    let max_bytes = if source_read {
        256 * 1024
    } else {
        MAX_FILE_BYTES as usize
    };
    let mut reader = io::BufReader::new(file);
    let mut result = String::new();
    let mut number = 0u64;
    loop {
        if number >= start && number >= end.saturating_sub(1) {
            break;
        }
        let selected = number.saturating_add(1) >= start;
        let remaining = if selected {
            max_bytes.saturating_sub(result.len().saturating_add(64))
        } else {
            0
        };
        let Some((bytes, clipped)) = bounded_task_line(&mut reader, remaining)? else {
            break;
        };
        number = number.saturating_add(1);
        if number < start {
            continue;
        }
        if number >= end {
            break;
        }
        let text = String::from_utf8_lossy(&bytes);
        let line = text.trim_end_matches(['\r', '\n']);
        let entry = format!("{number}: {line}");
        if !result.is_empty() {
            result.push('\n');
        }
        if result.len().saturating_add(entry.len()) > max_bytes {
            result.push_str("[Task output range exceeds Read limit; use task_output_delta]");
            break;
        }
        result.push_str(&entry);
        if clipped {
            result.push_str(
                "\n[Long task output line truncated; use task_output_delta with a byte offset]",
            );
            break;
        }
    }
    Ok(result)
}

/// TaskOutputTool returns a structured result, not a truncated JSON
/// fragment. Keep status, offsets and file path intact when the transcript
/// exceeds the source tool's 100k-character model result budget.
fn model_task_result(output: String) -> String {
    const MAX_TASK_RESULT_CHARS: usize = 100_000;
    if output.chars().count() <= MAX_TASK_RESULT_CHARS {
        return output;
    }
    let mut value: Value = match serde_json::from_str(&output) {
        Ok(value) => value,
        Err(error) => return format!("ERROR: Invalid task result: {error}"),
    };
    let content = if let Some(content) = value.get("content").and_then(Value::as_str) {
        Some((content.to_owned(), false))
    } else {
        value
            .pointer("/task/output")
            .and_then(Value::as_str)
            .map(|content| (content.to_owned(), true))
    };
    if let Some((content, nested)) = content {
        if nested {
            value["task"]["contentTruncated"] = json!(true);
        } else {
            value["contentTruncated"] = json!(true);
        }
        value["outputHint"] = json!(
            "Full output remains on disk; use Read on output_file or task_output_delta with a smaller byte range"
        );
        // JSON escaping expands quotes and control characters. Fit as much
        // real content as possible under the source tool's result budget,
        // while preserving parseable metadata and a precise truncation flag.
        let mut lower = 0usize;
        let mut upper = content.chars().count().min(MAX_TASK_RESULT_CHARS);
        let mut best = String::new();
        while lower <= upper {
            let middle = lower + (upper - lower) / 2;
            let preview: String = content.chars().take(middle).collect();
            if nested {
                value["task"]["output"] = json!(preview);
            } else {
                value["content"] = json!(preview);
            }
            let serialized = value.to_string();
            if serialized.chars().count() <= MAX_TASK_RESULT_CHARS {
                best = serialized;
                lower = middle.saturating_add(1);
            } else if middle == 0 {
                break;
            } else {
                upper = middle - 1;
            }
        }
        return if best.is_empty() {
            value.to_string()
        } else {
            best
        };
    }
    value.to_string()
}

fn skill_invocable(path: &Path, for_model: bool) -> Result<bool> {
    let content = fs::read_to_string(path)?;
    if for_model {
        Ok(
            !crate::skill_arguments::frontmatter_boolean(&content, "disable-model-invocation")
                .unwrap_or(false),
        )
    } else {
        Ok(crate::skill_arguments::frontmatter_boolean(&content, "user-invocable").unwrap_or(true))
    }
}

/// The original Read/Edit contract is scoped to a conversation, not a
/// particular model turn. A partial Read cannot authorize a full-file Edit.
#[derive(Clone, Debug)]
pub struct FileReadState {
    pub content: String,
    pub timestamp_ms: u64,
    pub full: bool,
    /// Read range metadata is only present for an actual Read result.
    pub read_offset: Option<usize>,
    pub read_limit: Option<usize>,
}

/// Match the source FileStateCache's 100-entry / 25 MiB LRU limits.
/// A cache hit refreshes recency; removed entries never authorize writes.
#[derive(Default, Debug)]
pub struct FileReadCache {
    entries: HashMap<PathBuf, FileReadState>,
    recency: VecDeque<PathBuf>,
    bytes: usize,
}

impl FileReadCache {
    pub fn get(&mut self, path: &Path) -> Option<FileReadState> {
        let state = self.entries.get(path).cloned()?;
        self.recency.retain(|entry| entry.as_path() != path);
        self.recency.push_back(path.to_path_buf());
        Some(state)
    }

    pub fn insert(&mut self, path: PathBuf, state: FileReadState) {
        const MAX_ENTRIES: usize = 100;
        const MAX_BYTES: usize = 25 * 1024 * 1024;
        if state.content.len() > MAX_BYTES {
            // Source FileStateCache uses lru-cache.set(): an oversized
            // replacement invalidates the prior value for the same key.
            // Retaining a previously full Read would let Edit/Write rely on
            // stale authorization after the new large Read was not cached.
            if let Some(old) = self.entries.remove(&path) {
                self.bytes = self.bytes.saturating_sub(old.content.len().max(1));
            }
            self.recency.retain(|entry| entry != &path);
            return;
        }
        if let Some(old) = self.entries.remove(&path) {
            self.bytes = self.bytes.saturating_sub(old.content.len().max(1));
        }
        self.recency.retain(|entry| entry != &path);
        self.bytes = self.bytes.saturating_add(state.content.len().max(1));
        self.entries.insert(path.clone(), state);
        self.recency.push_back(path);
        while self.entries.len() > MAX_ENTRIES || self.bytes > MAX_BYTES {
            let Some(oldest) = self.recency.pop_front() else {
                break;
            };
            if let Some(removed) = self.entries.remove(&oldest) {
                self.bytes = self.bytes.saturating_sub(removed.content.len().max(1));
            }
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.recency.clear();
        self.bytes = 0;
    }
}

/// Source FileEditTool/utils.ts: match a model's straight quotes against
/// typographic quotes while returning the exact on-disk byte span.
fn normalized_quote(ch: char) -> char {
    match ch {
        '‘' | '’' => '\'',
        '“' | '”' => '"',
        other => other,
    }
}

fn actual_edit_text(content: &str, requested: &str) -> Option<String> {
    if content.contains(requested) {
        return Some(requested.to_owned());
    }
    let needle: Vec<char> = requested.chars().map(normalized_quote).collect();
    let haystack: Vec<(usize, char)> = content.char_indices().collect();
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    for start in 0..=haystack.len() - needle.len() {
        if needle
            .iter()
            .enumerate()
            .all(|(offset, expected)| normalized_quote(haystack[start + offset].1) == *expected)
        {
            let first = haystack[start].0;
            let end = haystack
                .get(start + needle.len())
                .map(|(index, _)| *index)
                .unwrap_or(content.len());
            return Some(content[first..end].to_owned());
        }
    }
    None
}

fn opening_quote_context(chars: &[char], index: usize) -> bool {
    index == 0
        || matches!(
            chars[index - 1],
            ' ' | '\t' | '\n' | '\r' | '(' | '[' | '{' | '—' | '–'
        )
}

fn preserve_curly_quotes(requested: &str, actual: &str, replacement: &str) -> String {
    if requested == actual {
        return replacement.to_owned();
    }
    let double = actual.contains('“') || actual.contains('”');
    let single = actual.contains('‘') || actual.contains('’');
    let chars: Vec<char> = replacement.chars().collect();
    chars
        .iter()
        .enumerate()
        .map(|(i, ch)| match *ch {
            '"' if double => {
                if opening_quote_context(&chars, i) {
                    '“'
                } else {
                    '”'
                }
            }
            '\'' if single => {
                if i > 0
                    && i + 1 < chars.len()
                    && chars[i - 1].is_alphabetic()
                    && chars[i + 1].is_alphabetic()
                {
                    '’'
                } else if opening_quote_context(&chars, i) {
                    '‘'
                } else {
                    '’'
                }
            }
            other => other,
        })
        .collect()
}

/// Keep Markdown hard line breaks, while matching the original edit-input
/// normalization for ordinary files.
fn normalize_edit_replacement(path: &Path, replacement: &str) -> String {
    let markdown = path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("mdx"));
    if markdown {
        return replacement.to_owned();
    }
    let mut result = String::new();
    let mut start = 0;
    let mut chars = replacement.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if ch != '\r' && ch != '\n' {
            continue;
        }
        result.push_str(replacement[start..index].trim_end_matches(char::is_whitespace));
        if ch == '\r' && chars.peek().is_some_and(|(_, next)| *next == '\n') {
            chars.next();
            result.push_str("\r\n");
            start = index + 2;
        } else {
            result.push(ch);
            start = index + ch.len_utf8();
        }
    }
    result.push_str(replacement[start..].trim_end_matches(char::is_whitespace));
    result
}

#[derive(Clone, Copy)]
enum TextEncoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
}

struct EditableText {
    content: String,
    encoding: TextEncoding,
    crlf: bool,
}

/// Original fileRead.ts decodes UTF-16LE BOMs and normalizes CRLF for the
/// read/edit state. Keep the disk representation separately so edits can
/// preserve encoding and the predominant original newline convention.
fn read_editable_text(path: &Path) -> Result<EditableText> {
    let bytes = fs::read(path)?;
    let (encoding, decoded) = if bytes.starts_with(&[0xff, 0xfe]) {
        let raw = &bytes[2..];
        if raw.len() % 2 != 0 {
            bail!(
                "UTF-16LE file contains an incomplete code unit: {}",
                path.display()
            );
        }
        let units = raw
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        (
            TextEncoding::Utf16Le,
            String::from_utf16(&units)
                .with_context(|| format!("Invalid UTF-16LE in {}", path.display()))?,
        )
    } else if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        (
            TextEncoding::Utf8Bom,
            String::from_utf8(bytes[3..].to_vec())
                .with_context(|| format!("Invalid UTF-8 in {}", path.display()))?,
        )
    } else {
        (
            TextEncoding::Utf8,
            String::from_utf8(bytes)
                .with_context(|| format!("Invalid UTF-8 in {}", path.display()))?,
        )
    };
    let sample: String = decoded.chars().take(4096).collect();
    let crlf_count = sample.matches("\r\n").count();
    let lf_count = sample.matches('\n').count().saturating_sub(crlf_count);
    Ok(EditableText {
        content: decoded.replace("\r\n", "\n"),
        encoding,
        crlf: crlf_count > lf_count,
    })
}

fn write_editable_text(
    path: &Path,
    content: &str,
    encoding: TextEncoding,
    crlf: Option<bool>,
) -> Result<()> {
    let text = match crlf {
        Some(true) => content.replace("\r\n", "\n").replace('\n', "\r\n"),
        Some(false) => content.replace("\r\n", "\n"),
        None => content.to_owned(), // FileWriteTool honors explicit model newlines.
    };
    match encoding {
        TextEncoding::Utf8 => fs::write(path, text.as_bytes())?,
        TextEncoding::Utf8Bom => {
            let mut bytes = vec![0xef, 0xbb, 0xbf];
            bytes.extend_from_slice(text.as_bytes());
            fs::write(path, bytes)?;
        }
        TextEncoding::Utf16Le => {
            let mut bytes = vec![0xff, 0xfe];
            for unit in text.encode_utf16() {
                bytes.extend_from_slice(&unit.to_le_bytes());
            }
            fs::write(path, bytes)?;
        }
    }
    Ok(())
}

fn modified_millis(metadata: &fs::Metadata) -> Result<u64> {
    Ok(metadata
        .modified()?
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .min(u128::from(u64::MAX)) as u64)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Approval {
    ReadOnly,
    Ask,
    Full,
}

#[derive(Clone)]
pub struct ToolExecutor {
    root: PathBuf,
    extra_roots: Vec<PathBuf>,
    shell_kind: crate::shell_runtime::ShellKind,
    session_env: std::collections::BTreeMap<String, String>,
    session_id: Option<String>,
    history_message_id: Option<uuid::Uuid>,
    approval: Approval,
    interactive: bool,
    file_checkpointing_enabled: bool,
    reads: Arc<Mutex<HashMap<PathBuf, u64>>>,
    file_reads: Arc<Mutex<FileReadCache>>,
    enabled_plugins: Vec<String>,
    search_backend: Option<SearchBackend>,
    background: Option<Arc<BackgroundTasks>>,
    permission_rules: Vec<permission_rules::Rule>,
    mode: Option<permission_mode::Mode>,
    metrics: Arc<ToolMetrics>,
}

impl ToolExecutor {
    pub fn new(root: PathBuf, approval: Approval, interactive: bool) -> Self {
        Self {
            root,
            extra_roots: Vec::new(),
            shell_kind: crate::shell_runtime::ShellKind::default(),
            session_env: std::collections::BTreeMap::new(),
            session_id: None,
            history_message_id: None,
            approval,
            interactive,
            file_checkpointing_enabled: true,
            reads: Arc::new(Mutex::new(HashMap::new())),
            file_reads: Arc::new(Mutex::new(FileReadCache::default())),
            enabled_plugins: Vec::new(),
            search_backend: None,
            background: None,
            permission_rules: Vec::new(),
            mode: None,
            metrics: Arc::new(ToolMetrics::default()),
        }
    }

    pub fn with_extra_roots(mut self, roots: Vec<PathBuf>) -> Self {
        self.extra_roots = roots;
        self
    }
    pub fn with_shell(mut self, kind: crate::shell_runtime::ShellKind) -> Self {
        self.shell_kind = kind;
        self
    }
    pub fn with_environment(mut self, env: std::collections::BTreeMap<String, String>) -> Self {
        self.session_env = env;
        self
    }
    pub fn with_file_checkpointing(mut self, enabled: bool) -> Self {
        self.file_checkpointing_enabled = enabled;
        self
    }
    pub fn with_session_id(mut self, session_id: String) -> Self {
        self.session_id = Some(session_id);
        self
    }
    /// Message-level SDK rewind shares the exact same path authorization,
    /// explicit Edit deny/ask rules and checkpoint integrity checks as the
    /// user-facing file_revert tool. Check all targets before changing any.
    pub fn sdk_rewind_message_records(
        &self,
        records: &[history::Record],
        dry_run: bool,
    ) -> Result<Value> {
        if !history::enabled(self.interactive, self.file_checkpointing_enabled) {
            bail!("File checkpointing is disabled");
        }
        let mut checked = Vec::with_capacity(records.len());
        for record in records {
            let path = self.checked_path(&record.path)?;
            if dry_run {
                self.require_read_rules(&record.path, &path)?;
            } else {
                let args = json!({
                    "path": record.path,
                    "paths": self.file_rule_paths(&record.path)?
                });
                match permission_rules::policy_for(&self.permission_rules, "edit_file", &args)? {
                    Some(permission_rules::Behavior::Deny) => {
                        bail!(
                            "Message rewind denied by explicit Edit rule: {}",
                            record.path
                        );
                    }
                    Some(permission_rules::Behavior::Ask) => {
                        self.require_approval("edit_file", &args)?;
                    }
                    _ => {}
                }
                self.require_approval("file_revert", &args)?;
            }
            checked.push((record.clone(), path));
        }
        history::preflight_message_rewind(&checked)?;
        if dry_run {
            return Ok(json!({
                "can_rewind":true,
                "files":checked.iter().map(|(record, _)| record.path.clone())
                    .collect::<Vec<_>>(),
                "checkpoints":checked.len()
            }));
        }
        let mut restored = Vec::new();
        for (record, path) in checked {
            restored.push(history::restore(&record, &path)?);
        }
        Ok(json!({"restored":restored,"checkpoints":records.len()}))
    }

    pub fn with_history_message_id(mut self, id: uuid::Uuid) -> Self {
        self.history_message_id = Some(id);
        self
    }
    pub fn with_metrics(mut self, metrics: Arc<ToolMetrics>) -> Self {
        self.metrics = metrics;
        self
    }

    pub fn with_shared_reads(
        mut self,
        notebook_reads: Arc<Mutex<HashMap<PathBuf, u64>>>,
        file_reads: Arc<Mutex<FileReadCache>>,
    ) -> Self {
        self.reads = notebook_reads;
        self.file_reads = file_reads;
        self
    }

    /// SDK seed_read_state: an observed read may have been removed from the
    /// transcript. Only accept it while the file is still at the observed
    /// modification time; never manufacture a read of newer content.
    pub fn seed_read_state(&self, requested: &str, observed_ms: u64) -> Result<()> {
        let path = self.checked_path(requested)?;
        self.require_read_rules(requested, &path)?;
        let before = match fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        let disk_ms = modified_millis(&before)?;
        if before.len() > MAX_FILE_BYTES || disk_ms > observed_ms {
            return Ok(());
        }
        let content = read_editable_text(&path)?.content;
        let after = fs::metadata(&path)?;
        if modified_millis(&after)? != disk_ms || after.len() != before.len() {
            return Ok(());
        }
        let key = path.canonicalize()?;
        let mut reads = self.file_reads.lock().unwrap();
        if reads
            .get(&key)
            .is_none_or(|existing| existing.timestamp_ms < disk_ms)
        {
            reads.insert(
                key,
                FileReadState {
                    content,
                    timestamp_ms: disk_ms,
                    full: true,
                    read_offset: None,
                    read_limit: None,
                },
            );
        }
        Ok(())
    }

    pub fn with_permission_mode(mut self, mode: permission_mode::Mode) -> Self {
        self.mode = Some(mode);
        self
    }

    pub fn with_permission_rules(mut self, rules: Vec<permission_rules::Rule>) -> Self {
        self.permission_rules = rules;
        self
    }
    pub fn with_background(mut self, background: Arc<BackgroundTasks>) -> Self {
        self.background = Some(background);
        self
    }

    pub fn with_search(mut self, backend: Option<SearchBackend>) -> Self {
        self.search_backend = backend;
        self
    }

    pub fn with_plugins(mut self, enabled: Vec<String>) -> Self {
        self.enabled_plugins = enabled;
        self
    }

    /// Validate before any PreToolUse hook can launch a subprocess. Serial,
    /// concurrent and streaming dispatch share this boundary; dispatch still
    /// validates the effective arguments after hook rewrites.
    pub fn validate_call_before_hooks(&self, call: &ToolCall) -> Result<()> {
        let value: Value = serde_json::from_str(&call.function.arguments)
            .context("Invalid tool arguments JSON")?;
        if !value.is_object() {
            bail!("Tool arguments must be a JSON object");
        }
        let name = crate::tool_names::canonical_name(&call.function.name);
        if matches!(call.function.name.as_str(), "Read" | "Edit" | "Write") {
            let mut normalized = value;
            crate::semantic_inputs::normalize_source_file_tool(
                &call.function.name,
                &mut normalized,
            )
            .and_then(|_| crate::semantic_inputs::normalize_tool_fields(name, &mut normalized))
            .with_context(|| format!("Invalid {} arguments", call.function.name))?;
        } else if matches!(name, "grep_files" | "glob_files") {
            crate::search::validate_tool_input(name, &value).context("Invalid search arguments")?;
        } else if crate::task_graph::handles(&call.function.name) {
            crate::task_graph::TaskGraph::validate_tool_input(&call.function.name, &value)
                .with_context(|| format!("Invalid {} arguments", call.function.name))?;
        } else if name == "LocalTrigger" {
            crate::local_triggers::validate_model_input(&value)
                .context("Invalid LocalTrigger input")?;
        } else if matches!(name, "task_stop" | "KillShell") {
            self.validate_task_stop(&value)
                .context("Invalid TaskStop arguments")?;
        } else if name == "task_output" {
            Self::validate_task_output(&value).context("Invalid TaskOutput arguments")?;
        } else if matches!(name, "run_command" | "PowerShell") {
            Self::validate_shell_input(name, &value).context("Invalid Shell arguments")?;
        }
        Ok(())
    }

    pub async fn execute(&self, call: &ToolCall) -> String {
        let shell = crate::tool_names::canonical_name(&call.function.name) == "run_command"
            || call.function.name == "PowerShell";
        if shell {
            crate::command_lifecycle::notify_command_lifecycle(
                &call.id,
                crate::command_lifecycle::State::Started,
            );
        }
        let result = match self.dispatch(call).await {
            Ok(output)
                if crate::tool_names::canonical_name(&call.function.name) == "web_fetch"
                    && output.chars().count() > MAX_OUTPUT_CHARS =>
            {
                match self.persist_large_web_fetch(&output) {
                    Ok(saved) => saved,
                    Err(err) => format!("ERROR: Could not preserve large WebFetch result: {err:#}"),
                }
            }
            // Original Read returns the complete requested text slice, with
            // its own file-size/token limits rather than generic tool clipping.
            Ok(output) if call.function.name == "Read" => output,
            Ok(output)
                if matches!(
                    call.function.name.as_str(),
                    "task_output" | "task_output_delta"
                ) =>
            {
                model_task_result(output)
            }
            Ok(output) => truncate(
                &output,
                if shell {
                    crate::shell_runtime::max_output_length()
                } else {
                    MAX_OUTPUT_CHARS
                },
            ),
            Err(err) => format!(
                "ERROR: {}",
                crate::tool_errors::format_error_text(&format!("{err:#}"))
            ),
        };
        if shell {
            crate::command_lifecycle::notify_command_lifecycle(
                &call.id,
                crate::command_lifecycle::State::Completed,
            );
        }
        result
    }

    /// Original toolResultStorage persists oversized results instead of
    /// silently discarding the rest of the model-visible content.
    fn persist_large_web_fetch(&self, output: &str) -> Result<String> {
        let result: Value = serde_json::from_str(output)?;
        let directory = self.root.join(".atlas").join("tool-results");
        fs::create_dir_all(&directory)?;
        let directory = directory.canonicalize()?;
        let root = self.root.canonicalize()?;
        if !directory.starts_with(&root) {
            bail!("WebFetch result storage escapes workspace");
        }
        let path = directory.join(format!("webfetch-result-{}.json", uuid::Uuid::new_v4()));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        file.write_all(output.as_bytes())?;
        file.sync_all()?;
        let relative = path.strip_prefix(&root).unwrap_or(&path);
        let url = result.get("url").and_then(Value::as_str).unwrap_or("");
        let code = result.get("code").and_then(Value::as_u64).unwrap_or(0);
        let preview = result.get("content").and_then(Value::as_str).unwrap_or("");
        let binary = result.get("persistedPath").and_then(Value::as_str);
        Ok(format!(
            "WebFetch {code} {url}\nFull response ({} bytes) saved to {}.\nUse run_command with a paged reader or JSON query to inspect the complete saved result.{}\n\nPreview:\n{}",
            output.len(),
            relative.display(),
            binary.map(|path| format!("\nBinary attachment: {path}")).unwrap_or_default(),
            truncate(preview, 1_000)
        ))
    }

    async fn dispatch(&self, call: &ToolCall) -> Result<String> {
        let mut args: Value = serde_json::from_str(&call.function.arguments)
            .with_context(|| format!("Invalid JSON arguments for {}", call.function.name))?;
        if !args.is_object() {
            bail!("Tool arguments must be a JSON object")
        }
        let name = crate::tool_names::canonical_name(&call.function.name);
        crate::semantic_inputs::normalize_source_file_tool(&call.function.name, &mut args)?;
        crate::semantic_inputs::normalize_tool_fields(name, &mut args)?;
        match name {
            "LocalTrigger" => crate::local_triggers::model_call(&self.root, &args, || {
                self.require_approval("LocalTrigger", &args)
            }),
            "read_file" => self.read_file_with_mode(&args, call.function.name == "Read"),
            "notebook_edit" => {
                self.require_approval("notebook_edit", &args)?;
                let path = self.checked_path(required(&args, "notebook_path")?)?;
                let track = history::enabled(self.interactive, self.file_checkpointing_enabled);
                let pending = track.then(|| history::capture_before_edit(&path));
                let result = self.notebook_edit(&args)?;
                let checkpoint_notice = pending
                    .map(|pending| {
                        history::finish_after_edit(
                            &self.root,
                            self.session_id.as_deref(),
                            &path,
                            pending,
                            self.history_message_id,
                        )
                    })
                    .unwrap_or_default();
                Ok(format!("{result}{checkpoint_notice}"))
            }
            "list_files" => self.list_files(&args),
            "glob_files" => {
                search::validate_tool_input("glob_files", &args)?;
                let raw = args.get("path").and_then(Value::as_str).unwrap_or(".");
                // An absolute glob overrides the default search directory.
                // Resolve its static prefix first, then authorize the actual
                // traversal root, including any explicitly added workspaces.
                let requested = args
                    .get("pattern")
                    .and_then(Value::as_str)
                    .and_then(search::absolute_glob_base)
                    .map(|base| base.to_string_lossy().into_owned())
                    .unwrap_or_else(|| raw.to_owned());
                let path = self.checked_path(&requested)?;
                self.require_read_rules(&requested, &path)?;
                search::glob_files_filtered(&self.root, &path, &args, &|candidate| {
                    self.search_read_not_denied(candidate)
                })
            }
            "grep_files" => {
                search::validate_tool_input("grep_files", &args)?;
                let raw = args.get("path").and_then(Value::as_str).unwrap_or(".");
                let path = self.checked_path(raw)?;
                // Original GrepTool checks read permission on its search root.
                self.require_read_rules(raw, &path)?;
                search::grep_files_filtered(&self.root, &path, &args, &|candidate| {
                    self.search_read_not_denied(candidate)
                })
            }
            "search_text" => self.search_text(&args),
            "list_skills" => self.list_skills(),
            "read_skill" => self.read_skill(&args),
            "web_search" => {
                self.require_approval("web_search", &args)?;
                let backend = self
                    .search_backend
                    .as_ref()
                    .context("No search backend configured")?;
                websearch::execute(backend, &args).await
            }
            "web_fetch" => {
                self.require_approval("web_fetch", &args)?;
                webfetch::execute_with_redirect_policy_in_workspace(&args, &self.root, &|target| {
                    // Check explicit denies now; a new tool invocation handles
                    // the redirected origin's interactive approval separately.
                    let policy = permission_rules::policy_for(
                        &self.permission_rules,
                        "web_fetch",
                        &json!({"url":target.as_str()}),
                    )?;
                    if policy == Some(permission_rules::Behavior::Deny) {
                        bail!("redirect destination explicitly denied");
                    }
                    Ok(())
                })
                .await
            }
            "git_status" => self.git_read(vec!["status".into(), "--short".into()]).await,
            "git_diff" => self.git_diff(&args).await,
            "git_log" => self.git_log(&args).await,
            "write_file" => {
                self.require_approval("write_file", &args)?;
                let path = self.checked_path(required(&args, "path")?)?;
                let track = history::enabled(self.interactive, self.file_checkpointing_enabled);
                let pending = track.then(|| history::capture_before_edit(&path));
                let result = self.write_file(&args).await?;
                let checkpoint_notice = pending
                    .map(|pending| {
                        history::finish_after_edit(
                            &self.root,
                            self.session_id.as_deref(),
                            &path,
                            pending,
                            self.history_message_id,
                        )
                    })
                    .unwrap_or_default();
                Ok(format!("{result}{checkpoint_notice}"))
            }
            "edit_file" => {
                self.require_approval("edit_file", &args)?;
                let path = self.checked_path(required(&args, "path")?)?;
                let track = history::enabled(self.interactive, self.file_checkpointing_enabled);
                let pending = track.then(|| history::capture_before_edit(&path));
                let result = self.edit_file(&args).await?;
                let checkpoint_notice = pending
                    .map(|pending| {
                        history::finish_after_edit(
                            &self.root,
                            self.session_id.as_deref(),
                            &path,
                            pending,
                            self.history_message_id,
                        )
                    })
                    .unwrap_or_default();
                Ok(format!("{result}{checkpoint_notice}"))
            }
            "file_history" => {
                if !history::enabled(self.interactive, self.file_checkpointing_enabled) {
                    bail!("File checkpointing is disabled")
                }
                history::list(&self.root, self.session_id.as_deref())
            }
            "file_history_diff" => {
                if !history::enabled(self.interactive, self.file_checkpointing_enabled) {
                    bail!("File checkpointing is disabled")
                }
                let record = history::load(
                    &self.root,
                    self.session_id.as_deref(),
                    required(&args, "id")?,
                )?;
                let path = self.checked_path(&record.path)?;
                // A diff preview reads the current file. Enforce the same
                // Read deny/ask rules as a normal Read before exposing stats.
                self.require_read_rules(&record.path, &path)?;
                history::preview(&record, &path)
            }
            "file_revert" => {
                if !history::enabled(self.interactive, self.file_checkpointing_enabled) {
                    bail!("File checkpointing is disabled")
                }
                let record = history::load(
                    &self.root,
                    self.session_id.as_deref(),
                    required(&args, "id")?,
                )?;
                let path = self.checked_path(&record.path)?;
                // Rewinding a file is a write. Reject a path-level Edit deny
                // before showing any approval prompt, including symlink targets.
                let edit_args = json!({
                    "path": record.path,
                    "paths": self.file_rule_paths(&record.path)?
                });
                let edit_decision =
                    permission_rules::policy_for(&self.permission_rules, "edit_file", &edit_args)?;
                if edit_decision == Some(permission_rules::Behavior::Deny) {
                    bail!("File revert denied by explicit Edit path rule");
                }
                self.require_approval("file_revert", &args)?;
                if edit_decision == Some(permission_rules::Behavior::Ask) {
                    self.require_approval("edit_file", &json!({"path": record.path}))?;
                }
                history::restore(&record, &path)
            }
            "file_rewind" => {
                if !history::enabled(self.interactive, self.file_checkpointing_enabled) {
                    bail!("File checkpointing is disabled");
                }
                let id = required(&args, "id")?;
                let plan = history::plan_rewind(&self.root, self.session_id.as_deref(), id)?;
                for entry in &plan.entries {
                    let path = self.checked_path(&entry.path)?;
                    let decision = permission_rules::policy_for(
                        &self.permission_rules,
                        "edit_file",
                        &json!({"path":entry.path,"paths":self.file_rule_paths(&entry.path)?}),
                    )?;
                    if decision == Some(permission_rules::Behavior::Deny) {
                        bail!(
                            "File rewind denied by explicit Edit path rule: {}",
                            entry.path
                        );
                    }
                    self.require_read_rules(&entry.path, &path)?;
                    if decision == Some(permission_rules::Behavior::Ask) {
                        self.require_approval("edit_file", &json!({"path":entry.path}))?;
                    }
                }
                self.require_approval("file_revert", &args)?;
                history::apply_rewind(&self.root, &plan)
            }
            "run_command" | "PowerShell" => {
                Self::validate_shell_input(name, &args)?;
                let kind = if call.function.name == "PowerShell" {
                    crate::shell_runtime::ShellKind::Powershell
                } else {
                    self.shell_kind
                };
                self.require_approval(&call.function.name, &args)?;
                if args
                    .get("run_in_background")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    let manager = self
                        .background
                        .as_ref()
                        .context("Background tasks unavailable")?;
                    manager
                        .start_with_shell_timeout(
                            required(&args, "command")?,
                            args.get("description")
                                .and_then(Value::as_str)
                                .unwrap_or("Shell command"),
                            kind,
                            Self::shell_timeout_ms(&args)?,
                        )
                        .await
                } else {
                    self.run_command_with_shell(&args, kind).await
                }
            }
            "background_list" => {
                let manager = self
                    .background
                    .as_ref()
                    .context("Background tasks unavailable")?;
                manager.list()
            }
            "task_output" => {
                let (id, block, timeout_ms) = Self::validate_task_output(&args)?;
                let manager = self
                    .background
                    .as_ref()
                    .context("Background tasks unavailable")?;
                manager.output(&id, block, timeout_ms).await
            }
            "task_output_delta" => {
                let manager = self
                    .background
                    .as_ref()
                    .context("Background tasks unavailable")?;
                manager.output_delta(
                    required(&args, "task_id")?,
                    args.get("from_offset").and_then(Value::as_u64).unwrap_or(0),
                    args.get("max_bytes")
                        .and_then(Value::as_u64)
                        .unwrap_or(8 * 1024 * 1024),
                )
            }
            "task_stop" | "KillShell" => {
                // The original TaskStop strict schema and validateInput run
                // before approval. A nonexistent or completed task must not
                // request an unnecessary permission grant.
                let id = self.validate_task_stop(&args)?;
                self.require_approval("task_stop", &args)?;
                let manager = self
                    .background
                    .as_ref()
                    .context("Background tasks unavailable")?;
                manager.stop(&id).await
            }
            other => bail!("Unknown tool: {other}"),
        }
    }

    /// Validate shell inputs before PreToolUse, following the source tool's
    /// strict schema and semanticBoolean/semanticNumber preprocessors.
    /// Runtime permission and command safety checks still run after approval.
    pub fn validate_shell_input(name: &str, args: &Value) -> Result<()> {
        let mut normalized = args.clone();
        crate::semantic_inputs::normalize_tool_fields(name, &mut normalized)?;
        let fields = normalized
            .as_object()
            .context("Shell arguments must be an object")?;
        if let Some(key) = fields.keys().find(|key| {
            !matches!(
                key.as_str(),
                "command" | "description" | "timeout" | "timeout_seconds" | "run_in_background"
            )
        }) {
            bail!("Unrecognized shell parameter {key}");
        }
        required(&normalized, "command")?;
        if normalized
            .get("description")
            .is_some_and(|value| !value.is_string())
        {
            bail!("Shell description must be a string");
        }
        if normalized
            .get("run_in_background")
            .is_some_and(|value| !value.is_boolean())
        {
            bail!("run_in_background must be a boolean");
        }
        if normalized
            .get("timeout_seconds")
            .is_some_and(|value| value.as_u64().is_none())
        {
            bail!("timeout_seconds must be a nonnegative integer");
        }
        if let Some(timeout) = normalized.get("timeout") {
            let value = timeout.as_f64().context("timeout must be a number")?;
            if !value.is_finite() || value < 0.0 {
                bail!("timeout must be a nonnegative number of milliseconds");
            }
        }
        Ok(())
    }

    /// TaskOutputTool uses strict arguments, semanticBoolean for block and
    /// an ordinary numeric timeout. Validate before PreToolUse so malformed
    /// output requests do not trigger hooks or background waiting.
    pub fn validate_task_output(args: &Value) -> Result<(String, bool, u64)> {
        let mut args = args.clone();
        crate::semantic_inputs::normalize_tool_fields("task_output", &mut args)?;
        let fields = args.as_object().context("Invalid task output arguments")?;
        if let Some(key) = fields
            .keys()
            .find(|key| !matches!(key.as_str(), "task_id" | "block" | "timeout"))
        {
            bail!("Unrecognized task output parameter {key}");
        }
        let id = required(&args, "task_id")?.to_owned();
        let block = match args.get("block") {
            None => true,
            Some(Value::Bool(value)) => *value,
            _ => bail!("task_output.block must be a boolean"),
        };
        let timeout_ms = match args.get("timeout") {
            None => 30_000,
            Some(Value::Number(value)) => {
                let number = value.as_f64().context("Invalid task output timeout")?;
                if !(0.0..=600_000.0).contains(&number) {
                    bail!("task_output.timeout must be between 0 and 600000 ms");
                }
                number as u64
            }
            _ => bail!("task_output.timeout must be a number"),
        };
        Ok((id, block, timeout_ms))
    }

    /// The original TaskStop schema and validateInput run before PreToolUse.
    /// Return the canonical target ID after a read-only status check.
    pub fn validate_task_stop(&self, args: &Value) -> Result<String> {
        let fields = args.as_object().context("Invalid TaskStop arguments")?;
        if let Some(key) = fields
            .keys()
            .find(|key| !matches!(key.as_str(), "task_id" | "shell_id"))
        {
            bail!("Unrecognized TaskStop parameter {key}");
        }
        for field in ["task_id", "shell_id"] {
            if args.get(field).is_some_and(|value| !value.is_string()) {
                bail!("{field} must be a string");
            }
        }
        let id = args
            .get("task_id")
            .and_then(Value::as_str)
            .or_else(|| args.get("shell_id").and_then(Value::as_str))
            .context("Missing required parameter: task_id")?;
        let manager = self
            .background
            .as_ref()
            .context("Background tasks unavailable")?;
        manager.validate_stop(id)?;
        Ok(id.to_owned())
    }

    pub fn approve_external(&self, call: &ToolCall) -> Result<()> {
        let args: Value = serde_json::from_str(&call.function.arguments)?;
        self.require_approval(&call.function.name, &args)
    }

    /// Delegating to a child does not itself grant filesystem write access:
    /// the child inherits this executor's approval mode. However an explicit
    /// Agent deny/ask rule must still apply even without worktree isolation.
    pub fn approve_delegation(&self, call: &ToolCall) -> Result<()> {
        let args: Value = serde_json::from_str(&call.function.arguments)?;
        match permission_rules::policy_for(&self.permission_rules, &call.function.name, &args)? {
            Some(permission_rules::Behavior::Deny) => bail!(
                "Tool {} denied by explicit permission rule",
                call.function.name
            ),
            Some(permission_rules::Behavior::Ask) => {
                if !self.interactive {
                    bail!(
                        "Tool {} needs approval; noninteractive delegation denied",
                        call.function.name
                    )
                }
                eprintln!(
                    "\nProposed {}: {}",
                    call.function.name,
                    truncate(&args.to_string(), 600)
                );
                eprint!("Allow delegation once? [y/N] ");
                io::stderr().flush()?;
                let mut answer = String::new();
                io::stdin().read_line(&mut answer)?;
                if answer.trim().eq_ignore_ascii_case("y")
                    || answer.trim().eq_ignore_ascii_case("yes")
                {
                    Ok(())
                } else {
                    bail!("User rejected {}", call.function.name)
                }
            }
            _ => Ok(()),
        }
    }

    /// Collect both the user's path and the actual filesystem target for
    /// Edit/Write rule checks. Existing symlinks and non-existent files under
    /// symlinked directories must not bypass a denied path pattern.
    fn file_rule_paths(&self, requested: &str) -> Result<Vec<String>> {
        let path = self.checked_path(requested)?;
        let mut ancestor = path.as_path();
        while !ancestor.exists() {
            ancestor = ancestor
                .parent()
                .context("File path has no existing ancestor")?;
        }
        let resolved = ancestor.canonicalize()?.join(path.strip_prefix(ancestor)?);
        let mut paths = vec![
            requested.to_owned(),
            path.to_string_lossy().into_owned(),
            resolved.to_string_lossy().into_owned(),
        ];
        for candidate in [&path, &resolved] {
            for root in std::iter::once(&self.root).chain(self.extra_roots.iter()) {
                if let Ok(relative) = candidate.strip_prefix(root) {
                    paths.push(relative.to_string_lossy().into_owned());
                }
            }
        }
        Ok(paths)
    }

    fn require_approval(&self, tool: &str, args: &Value) -> Result<()> {
        let mut security_approved = false;
        if crate::tool_names::canonical_name(tool) == "run_command" || tool == "PowerShell" {
            if let Some(command) = args.get("command").and_then(Value::as_str) {
                if let Some(reason) = crate::shell_quote::security_review_reason(command) {
                    if !self.interactive {
                        bail!("Shell command needs explicit security review: {reason}");
                    }
                    eprintln!("Security review required: {reason}");
                    eprintln!("Command: {}", truncate(command, 600));
                    eprint!("Approve this command once? [y/N] ");
                    io::stderr().flush()?;
                    let mut answer = String::new();
                    io::stdin().read_line(&mut answer)?;
                    if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
                        bail!("Shell command rejected: {reason}");
                    }
                    security_approved = true;
                }
            }
        }
        // Path rules are evaluated against the requested path and the
        // symlink-resolved target, including files not created yet.
        let mut permission_args = args.clone();
        let edit_path = if tool == "notebook_edit" {
            args.get("notebook_path").and_then(Value::as_str)
        } else if matches!(permission_rules::canonical_tool(tool), "Write" | "Edit") {
            args.get("path").and_then(Value::as_str)
        } else {
            None
        };
        if let Some(path) = edit_path {
            permission_args["path"] = json!(path);
            permission_args["paths"] = json!(self.file_rule_paths(path)?);
        }
        // PowerShell is a separate source tool with case-insensitive cmdlet
        // and alias permissions. Never evaluate its commands under Bash rules.
        let matched = if (crate::tool_names::canonical_name(tool) == "run_command"
            && self.shell_kind == crate::shell_runtime::ShellKind::Powershell)
            || tool == "PowerShell"
        {
            crate::powershell_permission_matching::policy_for(
                &self.permission_rules,
                args.get("command")
                    .and_then(Value::as_str)
                    .context("PowerShell command is missing")?,
            )?
        } else {
            permission_rules::policy_for(&self.permission_rules, tool, &permission_args)?
        };
        if matched == Some(permission_rules::Behavior::Deny) {
            bail!("Tool {tool} denied by explicit permission rule");
        }
        if self.approval == Approval::ReadOnly || self.mode == Some(permission_mode::Mode::Plan) {
            // Source BashTool/readOnlyValidation.ts validates read-only
            // queries instead of rejecting every shell tool. Keep this
            // local PR adaptation limited to fixed gh argv; no shell syntax,
            // templates, output files or mutations are authorized here.
            let query = (crate::tool_names::canonical_name(tool) == "run_command"
                || tool == "PowerShell")
                && args
                    .get("command")
                    .and_then(Value::as_str)
                    .and_then(crate::shell_runtime::read_only_pr_args)
                    .is_some()
                && args.get("run_in_background").and_then(Value::as_bool) != Some(true);
            if !query {
                if self.approval == Approval::ReadOnly {
                    bail!("Tool {tool} denied in read-only mode");
                }
                bail!("Tool {tool} denied by permission mode");
            }
            // Explicit ask rules still require the normal prompt below;
            // explicit deny rules have already been applied above.
            if matched != Some(permission_rules::Behavior::Ask) {
                return Ok(());
            }
        }
        if matched == Some(permission_rules::Behavior::Allow) {
            return Ok(());
        }
        if self.mode == Some(permission_mode::Mode::DontAsk) {
            bail!("Tool {tool} denied by permission mode");
        }
        if self.mode == Some(permission_mode::Mode::AcceptEdits)
            && matched != Some(permission_rules::Behavior::Ask)
            && matches!(permission_rules::canonical_tool(tool), "Write" | "Edit")
        {
            return Ok(());
        }
        if self.approval == Approval::Full && matched != Some(permission_rules::Behavior::Ask) {
            return Ok(());
        }
        if security_approved && matched != Some(permission_rules::Behavior::Ask) {
            return Ok(());
        }
        if !self.interactive {
            bail!("Tool {tool} needs approval; noninteractive mode requires an explicit allow rule or --approval full")
        }
        if crate::tool_names::canonical_name(tool) == "run_command" || tool == "PowerShell" {
            if let Some(command) = args.get("command").and_then(Value::as_str) {
                let warning = if tool == "PowerShell"
                    || self.shell_kind == crate::shell_runtime::ShellKind::Powershell
                {
                    crate::powershell_permission_matching::destructive_command_warning(command)
                } else {
                    crate::bash_permission_suggestions::destructive_command_warning(command)
                };
                if let Some(warning) = warning {
                    eprintln!("{warning}");
                }
            }
        }
        eprintln!("\nProposed {tool}: {}", truncate(&args.to_string(), 600));
        eprint!("Allow once? [y/N] ");
        io::stderr().flush()?;
        let mut answer = String::new();
        io::stdin().read_line(&mut answer)?;
        if answer.trim().eq_ignore_ascii_case("y") || answer.trim().eq_ignore_ascii_case("yes") {
            Ok(())
        } else {
            bail!("User rejected {tool}")
        }
    }

    /// Resolve paths lexically and check the nearest real ancestor against root.
    /// Existing symlinks that lead outside the workspace are rejected.
    fn checked_path(&self, raw: &str) -> Result<PathBuf> {
        crate::workspace_dirs::checked_path(raw, &self.root, &self.extra_roots)
    }

    /// File reads remain available in read-only/plan mode, but explicit Read
    /// deny and ask rules must be checked before opening the file. The source
    /// filesystem permission engine checks both the requested path and the
    /// symlink-resolved target; matching only the alias leaks denied content.
    fn read_rule_decision(
        &self,
        requested: &str,
        path: &Path,
    ) -> Result<Option<permission_rules::Behavior>> {
        let resolved = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let mut paths = vec![
            requested.to_owned(),
            path.to_string_lossy().into_owned(),
            resolved.to_string_lossy().into_owned(),
        ];
        for candidate in [path, &resolved] {
            for root in std::iter::once(&self.root).chain(self.extra_roots.iter()) {
                if let Ok(relative) = candidate.strip_prefix(root) {
                    paths.push(relative.to_string_lossy().into_owned());
                }
            }
        }
        permission_rules::policy_for(
            &self.permission_rules,
            "read_file",
            &json!({"path":requested,"paths":paths}),
        )
    }

    /// Original Glob/Grep apply Read deny patterns to descendant files;
    /// an Ask rule is handled at the search-root approval boundary instead
    /// of silently hiding individual matches after that approval.
    fn search_read_not_denied(&self, path: &Path) -> Result<bool> {
        Ok(!matches!(
            self.read_rule_decision(&path.to_string_lossy(), path)?,
            Some(permission_rules::Behavior::Deny)
        ))
    }

    fn search_read_allowed(&self, path: &Path) -> Result<bool> {
        Ok(!matches!(
            self.read_rule_decision(&path.to_string_lossy(), path)?,
            Some(permission_rules::Behavior::Deny | permission_rules::Behavior::Ask)
        ))
    }

    pub(crate) fn require_read_rules(&self, requested: &str, path: &Path) -> Result<()> {
        match self.read_rule_decision(requested, path)? {
            Some(permission_rules::Behavior::Deny) => {
                bail!("Read denied by explicit permission rule: {requested}")
            }
            Some(permission_rules::Behavior::Ask) => {
                if !self.interactive {
                    bail!("Read needs approval; noninteractive mode denied: {requested}");
                }
                eprintln!("Proposed read: {requested}");
                eprint!("Allow once? [y/N] ");
                io::stderr().flush()?;
                let mut answer = String::new();
                io::stdin().read_line(&mut answer)?;
                if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
                    bail!("User rejected read: {requested}");
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn read_file(&self, args: &Value) -> Result<String> {
        self.read_file_with_mode(args, false)
    }

    fn read_file_with_mode(&self, args: &Value, source_read: bool) -> Result<String> {
        let requested = required(args, "path")?;
        let trusted = match self.background.as_ref() {
            Some(background) => background.trusted_output_path(requested)?,
            None => None,
        };
        let trusted_task_output = trusted.is_some();
        let path = match trusted {
            Some(path) => path,
            None => self.checked_path(requested)?,
        };
        self.require_read_rules(requested, &path)?;
        if trusted_task_output {
            // Reading a temporary task log must never grant Edit/Write
            // authority or insert a full-file edit cache entry.
            return read_trusted_task_output(&path, args, source_read);
        }
        let metadata = fs::metadata(&path)?;
        // The original FileReadTool returns a small unchanged marker when
        // the same range was already shown and its mtime is unchanged.
        // Edited/written/SDK-seeded cache entries have no read_offset and
        // must never deduplicate against an old tool result.
        let requested_offset = args
            .get("start_line")
            .and_then(Value::as_u64)
            .unwrap_or(1)
            .max(1)
            .min(usize::MAX as u64) as usize;
        let requested_limit = args
            .get("line_count")
            .and_then(Value::as_u64)
            .map(|n| n.min(usize::MAX as u64) as usize);
        if source_read {
            let key = path.canonicalize()?;
            let disk_ms = modified_millis(&metadata)?;
            let prior = self.file_reads.lock().unwrap().get(&key);
            if prior.is_some_and(|state| {
                state.read_offset == Some(requested_offset)
                    && state.read_limit == requested_limit
                    && state.timestamp_ms == disk_ms
            }) {
                return Ok(format!("File unchanged since last read: {requested}"));
            }
        }
        let max_bytes = if source_read {
            256 * 1024
        } else {
            MAX_FILE_BYTES
        };
        if metadata.len() > max_bytes {
            bail!("File too large (limit {max_bytes} bytes)");
        }
        let content = read_editable_text(&path)
            .with_context(|| format!("Reading {}", path.display()))?
            .content;
        self.reads
            .lock()
            .unwrap()
            .insert(path.canonicalize()?, notebook::revision(&content));
        let start = args
            .get("start_line")
            .and_then(Value::as_u64)
            .unwrap_or(1)
            .max(1) as usize;
        let count = if source_read {
            args.get("line_count")
                .and_then(Value::as_u64)
                .map(|n| n.min(usize::MAX as u64) as usize)
                .unwrap_or(usize::MAX)
        } else {
            args.get("line_count")
                .and_then(Value::as_u64)
                .unwrap_or(250)
                .clamp(1, 500) as usize
        };
        let result = content
            .lines()
            .enumerate()
            .skip(start - 1)
            .take(count)
            .map(|(i, line)| format!("{}: {}", i + 1, line))
            .collect::<Vec<_>>()
            .join("\n");
        let after = fs::metadata(&path)?;
        let before_ms = modified_millis(&metadata)?;
        let unchanged = modified_millis(&after)? == before_ms && after.len() == metadata.len();
        self.file_reads.lock().unwrap().insert(
            path.canonicalize()?,
            FileReadState {
                full: unchanged
                    && start == 1
                    && content.lines().count() <= count
                    && (source_read || result.chars().count() <= MAX_OUTPUT_CHARS),
                content,
                timestamp_ms: before_ms,
                read_offset: if source_read && unchanged {
                    Some(requested_offset)
                } else {
                    None
                },
                read_limit: if source_read && unchanged {
                    requested_limit
                } else {
                    None
                },
            },
        );
        Ok(result)
    }

    fn notebook_edit(&self, args: &Value) -> Result<String> {
        let raw = required(args, "notebook_path")?;
        let path = self.checked_path(raw)?;
        let key = path.canonicalize()?;
        let known = self.reads.lock().unwrap().get(&key).copied();
        let (result, revision) = notebook::edit(&path, args, known)?;
        self.reads.lock().unwrap().insert(key, revision);
        Ok(result)
    }

    fn list_files(&self, args: &Value) -> Result<String> {
        let rel = args.get("path").and_then(Value::as_str).unwrap_or(".");
        let path = self.checked_path(rel)?;
        let depth = args
            .get("max_depth")
            .and_then(Value::as_u64)
            .unwrap_or(5)
            .clamp(1, 20) as usize;
        let limit = args
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(200)
            .clamp(1, 1000) as usize;
        let mut entries = Vec::new();
        for entry in WalkDir::new(&path)
            .max_depth(depth)
            .into_iter()
            .filter_entry(in_scope)
        {
            let entry = entry?;
            if entry.path() == path {
                continue;
            }
            if !self.search_read_allowed(entry.path())? {
                continue;
            }
            entries.push(
                entry
                    .path()
                    .strip_prefix(&self.root)
                    .unwrap_or(entry.path())
                    .display()
                    .to_string(),
            );
            if entries.len() >= limit {
                break;
            }
        }
        Ok(entries.join("\n"))
    }

    fn search_text(&self, args: &Value) -> Result<String> {
        let pattern = required(args, "pattern")?;
        let regex = Regex::new(pattern)?;
        let path = self.checked_path(args.get("path").and_then(Value::as_str).unwrap_or("."))?;
        let limit = args
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(80)
            .clamp(1, 300) as usize;
        let mut matches = Vec::new();
        for entry in WalkDir::new(path).into_iter().filter_entry(in_scope) {
            let entry = entry?;
            if !entry.file_type().is_file() {
                continue;
            }
            if !self.search_read_allowed(entry.path())? {
                continue;
            }
            if entry.metadata()?.len() > MAX_FILE_BYTES {
                continue;
            }
            let Ok(data) = fs::read_to_string(entry.path()) else {
                continue;
            };
            for (line_no, line) in data.lines().enumerate() {
                if regex.is_match(line) {
                    matches.push(format!(
                        "{}:{}: {}",
                        entry
                            .path()
                            .strip_prefix(&self.root)
                            .unwrap_or(entry.path())
                            .display(),
                        line_no + 1,
                        truncate(line, 300)
                    ));
                    if matches.len() >= limit {
                        return Ok(matches.join("\n"));
                    }
                }
            }
        }
        Ok(matches.join("\n"))
    }

    pub fn list_skills(&self) -> Result<String> {
        self.list_skills_filtered(true)
    }

    /// Interactive /skills lists user-invocable skills independently from the
    /// model's SkillTool allowlist. A user-only skill must remain discoverable.
    pub fn list_user_skills(&self) -> Result<String> {
        self.list_skills_filtered(false)
    }

    fn list_skills_filtered(&self, for_model: bool) -> Result<String> {
        let root = self.root.join(".atlas").join("skills");
        let mut skills = Vec::new();
        if root.is_dir() {
            for entry in fs::read_dir(root)? {
                let entry = entry?;
                let name = entry.file_name().to_string_lossy().to_string();
                if !valid_skill_name(&name) {
                    continue;
                }
                let path = self.checked_path(&format!(".atlas/skills/{name}/SKILL.md"))?;
                if path.is_file()
                    && self.search_read_allowed(&path)?
                    && skill_invocable(&path, for_model)?
                {
                    skills.push(name);
                }
            }
        }
        for name in plugins::list_skills(&self.root, &self.enabled_plugins)? {
            let resolved = plugins::resolve_skill_path(&self.root, &self.enabled_plugins, &name)?;
            let path = self.checked_path(&resolved.to_string_lossy())?;
            if self.search_read_allowed(&path)? && skill_invocable(&path, for_model)? {
                skills.push(name);
            }
        }
        skills.sort();
        Ok(skills.join("\n"))
    }

    fn read_skill(&self, args: &Value) -> Result<String> {
        self.render_skill(args, true)
    }

    /// User-invoked slash skills and model SkillTool calls have independent
    /// frontmatter permission switches in the original skill loader.
    pub fn invoke_user_skill(&self, name: &str, arguments: &str) -> Result<String> {
        self.render_skill(&json!({"name":name,"arguments":arguments}), false)
    }
    /// Agent frontmatter preloading uses model-invocable skill semantics,
    /// independently of whether a skill is user-invocable via a slash command.
    pub fn preload_agent_skill(&self, name: &str) -> Result<String> {
        self.render_skill(&json!({"name":name,"arguments":""}), true)
    }

    fn render_skill(&self, args: &Value, for_model: bool) -> Result<String> {
        let name = required(args, "name")?;
        let (content, skill_dir) = if name.contains(':') {
            let resolved = plugins::resolve_skill_path(&self.root, &self.enabled_plugins, name)?;
            let raw = resolved.to_string_lossy().into_owned();
            let path = self.checked_path(&raw)?;
            self.require_read_rules(&raw, &path)?;
            let dir = path
                .parent()
                .context("Skill has no parent directory")?
                .to_path_buf();
            (fs::read_to_string(path)?, dir)
        } else {
            if !valid_skill_name(name) {
                bail!("Invalid skill name")
            }
            let raw = format!(".atlas/skills/{name}/SKILL.md");
            let path = self.checked_path(&raw)?;
            self.require_read_rules(&raw, &path)?;
            let dir = path
                .parent()
                .context("Skill has no parent directory")?
                .to_path_buf();
            (fs::read_to_string(path)?, dir)
        };
        if for_model {
            if crate::skill_arguments::frontmatter_boolean(&content, "disable-model-invocation")
                .unwrap_or(false)
            {
                bail!("Skill cannot be invoked by the model")
            }
        } else if !crate::skill_arguments::frontmatter_boolean(&content, "user-invocable")
            .unwrap_or(true)
        {
            bail!("Skill cannot be invoked by the user")
        }
        let raw = args.get("arguments");
        let arguments = raw
            .map(|v| v.as_str().context("arguments must be a string"))
            .transpose()?;
        let names = if let Some(names) = args.get("argument_names") {
            crate::skill_arguments::parse_argument_names(Some(names))
        } else {
            crate::skill_arguments::frontmatter_argument_names(&content)
        };
        // Source loadSkillsDir.ts::getPromptForCommand supplies the
        // Markdown body and skill-local base directory, never YAML metadata.
        let dir = skill_dir.to_string_lossy().replace('\\', "/");
        let body = crate::skill_arguments::markdown_body(&content);
        let prompt = format!("Base directory for this skill: {dir}\n\n{body}");
        let rendered = crate::skill_arguments::substitute_arguments(
            &prompt,
            arguments,
            args.get("append_if_no_placeholder")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            &names,
        );
        // Keep original plugin/skill macros usable with the neutral runtime.
        // Resolve the exact plugin root already validated by skill discovery.
        let rendered = rendered
            .replace("${SKILL_DIR}", &dir)
            .replace("${ATLAS_SKILL_DIR}", &dir);
        let rendered = if let Some((plugin, _)) = name.split_once(':') {
            let root = self
                .root
                .join(".atlas/plugins")
                .join(plugin)
                .canonicalize()?;
            let root = root.to_string_lossy().replace('\\', "/");
            rendered.replace("${ATLAS_PLUGIN_ROOT}", &root)
        } else {
            rendered
        };
        let rendered = if name.contains(':') {
            plugins::expand_skill_user_config(&self.root, &self.enabled_plugins, name, &rendered)?
        } else {
            rendered
        };
        Ok(match self.session_id.as_deref() {
            Some(id) => rendered
                .replace("${SESSION_ID}", id)
                .replace("${ATLAS_SESSION_ID}", id),
            None => rendered,
        })
    }

    async fn git_read(&self, arguments: Vec<String>) -> Result<String> {
        let mut command = Command::new("git");
        crate::subprocess_env::apply(&mut command);
        command
            .arg("-C")
            .arg(&self.root)
            .arg("--no-pager")
            .args(arguments)
            .kill_on_drop(true);
        let output = timeout(Duration::from_secs(30), command.output())
            .await
            .context("Git command timed out")??;
        if !output.status.success() {
            bail!("Git error: {}", String::from_utf8_lossy(&output.stderr));
        }
        Ok(truncate(
            &String::from_utf8_lossy(&output.stdout),
            MAX_OUTPUT_CHARS,
        ))
    }

    async fn git_diff(&self, args: &Value) -> Result<String> {
        let mut command = vec![
            "diff".into(),
            "--no-ext-diff".into(),
            "--no-textconv".into(),
            "--no-color".into(),
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
        let count = args
            .get("count")
            .and_then(Value::as_u64)
            .unwrap_or(5)
            .clamp(1, 30);
        self.git_read(vec!["log".into(), format!("-{count}"), "--oneline".into()])
            .await
    }

    async fn write_file(&self, args: &Value) -> Result<String> {
        let path = self.checked_path(required(args, "path")?)?;
        let content = required(args, "content")?;
        let existed = path.exists();
        let create_only = args
            .get("create_only")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if content.len() as u64 > MAX_FILE_BYTES {
            bail!("Content too large")
        }
        if existed && create_only {
            bail!("File already exists; create_only forbids overwriting it");
        }
        let before =
            if existed {
                let original = read_editable_text(&path)?.content;
                let key = path.canonicalize()?;
                let prior =
                    self.file_reads.lock().unwrap().get(&key).context(
                        "File has not been read yet. Read it first before writing to it.",
                    )?;
                if !prior.full {
                    bail!("File was only partially read. Read the complete file before writing");
                }
                if original != prior.content {
                    bail!("File has been modified since read. Read it again before writing");
                }
                Some(original)
            } else {
                None
            };
        // Keep the final content check and write adjacent. The original
        // FileWriteTool likewise avoids an async suspension in this interval.
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        if create_only {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)?;
            file.write_all(content.as_bytes())?;
        } else {
            let encoding = if existed {
                read_editable_text(&path)?.encoding
            } else {
                TextEncoding::Utf8
            };
            write_editable_text(&path, content, encoding, None)?;
        }
        if !existed || before.is_some() {
            self.metrics.record_change(before.as_deref(), content);
        }
        let metadata = fs::metadata(&path)?;
        self.file_reads.lock().unwrap().insert(
            path.canonicalize()?,
            FileReadState {
                content: content.replace("\r\n", "\n"),
                timestamp_ms: modified_millis(&metadata)?,
                full: true,
                read_offset: None,
                read_limit: None,
            },
        );
        Ok(format!(
            "Wrote {} bytes to {}",
            content.len(),
            path.display()
        ))
    }

    async fn edit_file(&self, args: &Value) -> Result<String> {
        let path = self.checked_path(required(args, "path")?)?;
        let old = required(args, "old")?;
        let new = required(args, "new")?;
        if old.is_empty() {
            bail!("Replacement target cannot be empty")
        }
        if old == new {
            bail!("No changes to make: old and new strings are exactly the same")
        }
        if fs::metadata(&path)?.len() > MAX_FILE_BYTES {
            bail!("File too large")
        }
        // Validate against the complete content previously shown to the
        // model, including reads made in earlier turns of this session.
        let original_text = read_editable_text(&path)?;
        let content = original_text.content.as_str();
        let key = path.canonicalize()?;
        let prior = self
            .file_reads
            .lock()
            .unwrap()
            .get(&key)
            .context("File has not been read yet. Read it first before writing to it.")?;
        if !prior.full {
            bail!("File was only partially read. Read the complete file before editing");
        }
        if content != prior.content {
            bail!("File has been modified since read. Read it again before editing");
        }
        let actual_old = actual_edit_text(&content, old).unwrap_or_else(|| old.to_owned());
        let hits = content.matches(actual_old.as_str()).count();
        let normalized_new = normalize_edit_replacement(&path, new);
        let replacement = preserve_curly_quotes(old, &actual_old, &normalized_new);
        let replace_all = args
            .get("replace_all")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if hits == 0 || (hits != 1 && !replace_all) {
            bail!("Expected exactly one match; found {hits}")
        }
        let updated = if replace_all {
            content.replace(actual_old.as_str(), replacement.as_str())
        } else {
            content.replacen(actual_old.as_str(), replacement.as_str(), 1)
        };
        if updated.len() as u64 > MAX_FILE_BYTES {
            bail!("Updated file too large")
        }
        write_editable_text(
            &path,
            &updated,
            original_text.encoding,
            Some(original_text.crlf),
        )?;
        self.metrics.record_change(Some(content), &updated);
        let metadata = fs::metadata(&path)?;
        self.file_reads.lock().unwrap().insert(
            key,
            FileReadState {
                content: updated,
                timestamp_ms: modified_millis(&metadata)?,
                full: true,
                read_offset: None,
                read_limit: None,
            },
        );
        Ok(format!(
            "Replaced {hits} occurrence(s) in {}",
            path.display()
        ))
    }

    /// Backgrounded commands retain the same original BashTool timeout
    /// semantics as foreground commands, including explicit milliseconds.
    fn shell_timeout_ms(args: &Value) -> Result<u64> {
        let env = std::env::vars().collect::<HashMap<String, String>>();
        // 2.1.88 BashTool and PowerShellTool accept timeout in milliseconds.
        // The provider-neutral timeout_seconds field retains its seconds unit.
        // Keeping the deadline in milliseconds avoids silently rounding a
        // short original Bash timeout up to a whole second.
        let default_ms = crate::portable_source_utils::default_bash_timeout_ms(&env);
        let requested_ms = if let Some(value) = args.get("timeout_seconds") {
            value
                .as_u64()
                .context("timeout_seconds must be a nonnegative integer")?
                .saturating_mul(1000)
                .max(1000)
        } else if let Some(value) = args.get("timeout") {
            let milliseconds = value.as_f64().context("timeout must be a number")?;
            if !milliseconds.is_finite() || milliseconds < 0.0 {
                bail!("timeout must be a nonnegative number of milliseconds");
            }
            if milliseconds == 0.0 {
                default_ms
            } else {
                milliseconds.max(1.0).trunc() as u64
            }
        } else {
            default_ms
        };
        let timeout_ms =
            requested_ms.clamp(1, crate::portable_source_utils::max_bash_timeout_ms(&env));
        Ok(timeout_ms)
    }

    async fn run_command_with_shell(
        &self,
        args: &Value,
        kind: crate::shell_runtime::ShellKind,
    ) -> Result<String> {
        self.run_command_with_shell_limit(args, kind, 5 * 1024 * 1024 * 1024)
            .await
    }

    async fn run_command_with_shell_limit(
        &self,
        args: &Value,
        kind: crate::shell_runtime::ShellKind,
        hard_output_limit: u64,
    ) -> Result<String> {
        let command = required(args, "command")?;
        let timeout_ms = Self::shell_timeout_ms(args)?;
        let pr_args = if self.approval == Approval::ReadOnly
            || self.mode == Some(permission_mode::Mode::Plan)
        {
            crate::shell_runtime::read_only_pr_args(command)
        } else {
            None
        };
        let read_only_query = pr_args.is_some();
        let mut proc = if let Some(args) = pr_args {
            let mut proc = tokio::process::Command::new("gh");
            proc.args(args);
            proc
        } else {
            kind.build_command(command)?
        };
        proc.envs(&self.session_env);
        crate::subprocess_env::apply(&mut proc);
        if read_only_query {
            // Enforce this after applying session environment overrides.
            proc.env("GH_PAGER", "cat").env("PAGER", "cat");
        }
        // The original ShellCommand uses tree-kill on timeout and abort.
        // A dropped shell process alone leaves descendants alive, possibly
        // holding stdout/stderr pipes and continuing unauthorized side effects.
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(proc.as_std_mut(), 0);
        proc.current_dir(&self.root)
            .kill_on_drop(true)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = proc.spawn()?;
        let process_id = child.id();
        // Tokio's kill_on_drop kills only the direct shell child. A cancelled
        // SDK run drops this future without entering the timeout/error arms.
        // Own the entire Unix process group until the shell and pipe readers
        // have completed, so descendants cannot outlive user cancellation.
        #[cfg(unix)]
        let mut cancel_group_guard = ShellCancelGroupGuard(process_id);
        #[cfg(windows)]
        let mut cancel_tree_guard = ShellCancelTreeGuard(process_id);
        let stdout = child.stdout.take().context("No command stdout")?;
        let stderr = child.stderr.take().context("No command stderr")?;
        let combined_output_bytes = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let operation = timeout(Duration::from_millis(timeout_ms), async {
            let (out, err) = tokio::try_join!(
                capped_read(
                    stdout,
                    crate::shell_runtime::max_output_length().saturating_mul(4),
                    combined_output_bytes.clone(),
                    hard_output_limit
                ),
                capped_read(
                    stderr,
                    32_000,
                    combined_output_bytes.clone(),
                    hard_output_limit
                )
            )?;
            let status = child.wait().await?;
            Ok::<_, anyhow::Error>((status, out, err))
        })
        .await;
        let (status, out, err) = match operation {
            Ok(Ok(output)) => output,
            Ok(Err(error)) => {
                // Readers can fail (including output-size exhaustion) while
                // descendants are still running: terminate the whole tree.
                #[cfg(unix)]
                if let Some(pid) = process_id {
                    let _ = unsafe { libc::kill(-(pid as i32), libc::SIGKILL) };
                }
                #[cfg(windows)]
                let tree_error = crate::shell_runtime::terminate_windows_process_tree(process_id)
                    .await
                    .err();
                let _ = child.kill().await;
                #[cfg(windows)]
                if let Some(tree_error) = tree_error {
                    return Err(anyhow::anyhow!(
                        "Command output failed: {error:#}; process-tree termination failed: {tree_error:#}"
                    ));
                }
                return Err(error);
            }
            Err(_) => {
                #[cfg(unix)]
                if let Some(pid) = process_id {
                    let _ = unsafe { libc::kill(-(pid as i32), libc::SIGKILL) };
                }
                #[cfg(windows)]
                let tree_error = crate::shell_runtime::terminate_windows_process_tree(process_id)
                    .await
                    .err();
                let _ = child.kill().await;
                #[cfg(windows)]
                if let Some(tree_error) = tree_error {
                    bail!(
                        "Command timed out after {timeout_ms} milliseconds; process-tree termination failed: {tree_error:#}"
                    );
                }
                bail!("Command timed out after {timeout_ms} milliseconds");
            }
        };
        #[cfg(unix)]
        {
            cancel_group_guard.0 = None;
        }
        #[cfg(windows)]
        {
            cancel_tree_guard.0 = None;
        }
        let interpretation = if kind == crate::shell_runtime::ShellKind::Bash {
            let (is_error, note) =
                crate::shell_runtime::interpret_bash_exit(command, status.code().unwrap_or(-1));
            match (is_error, note) {
                (_, Some(note)) => format!("Interpretation: {note}\n"),
                (true, None) => format!(
                    "Interpretation: Command failed with exit code {}\n",
                    status.code().unwrap_or(-1)
                ),
                (false, None) => String::new(),
            }
        } else {
            String::new()
        };
        let stdout_omitted = out.1.saturating_sub(out.0.len() as u64);
        let stderr_omitted = err.1.saturating_sub(err.0.len() as u64);
        let stdout_notice = if stdout_omitted > 0 {
            format!("[stdout preview only: {stdout_omitted} bytes omitted]\n")
        } else {
            String::new()
        };
        let stderr_notice = if stderr_omitted > 0 {
            format!("[stderr preview only: {stderr_omitted} bytes omitted]\n")
        } else {
            String::new()
        };
        Ok(format!(
            "Exit: {}\n{}STDOUT:\n{}{}\nSTDERR:\n{}{}",
            status,
            interpretation,
            stdout_notice,
            truncate(
                &String::from_utf8_lossy(&out.0),
                crate::shell_runtime::max_output_length()
            ),
            stderr_notice,
            truncate(&String::from_utf8_lossy(&err.0), 4_000)
        ))
    }
}

// A ShellCommand that is cancelled while awaiting I/O never reaches its
// timeout/error arms. Retain process-group ownership until normal completion.
// This mirrors the existing hook cancellation guard and source tree-kill.
#[cfg(unix)]
struct ShellCancelGroupGuard(Option<u32>);

#[cfg(unix)]
impl Drop for ShellCancelGroupGuard {
    fn drop(&mut self) {
        if let Some(pid) = self.0.filter(|id| *id <= i32::MAX as u32) {
            let _ = unsafe { libc::kill(-(pid as i32), libc::SIGKILL) };
        }
    }
}

/// The Windows equivalent of ShellCancelGroupGuard: process cancellation
/// drops the model/tool future without running the timeout handler. Invoke
/// the platform's existing taskkill tree operation synchronously from Drop.
#[cfg(windows)]
struct ShellCancelTreeGuard(Option<u32>);

#[cfg(windows)]
impl Drop for ShellCancelTreeGuard {
    fn drop(&mut self) {
        if let Some(pid) = self.0 {
            let _ = std::process::Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/T", "/F"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
}

/// Keep a bounded preview but continue draining both pipes. The original
/// ShellCommand/TaskOutput only displays an initial output slice; a command
/// producing more than that slice must still finish and return its exit code.
/// The original 5 GiB hard disk-output limit remains a termination condition.
async fn capped_read<R: tokio::io::AsyncRead + Unpin>(
    mut input: R,
    max_bytes: usize,
    combined_bytes: Arc<std::sync::atomic::AtomicU64>,
    hard_limit: u64,
) -> Result<(Vec<u8>, u64)> {
    use std::sync::atomic::Ordering;
    use tokio::io::AsyncReadExt;
    let mut bytes = Vec::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 8192];
    loop {
        let n = input.read(&mut buffer).await?;
        if n == 0 {
            break;
        }
        total = total.saturating_add(n as u64);
        let combined = combined_bytes
            .fetch_add(n as u64, Ordering::Relaxed)
            .saturating_add(n as u64);
        if combined > hard_limit {
            bail!("Command output exceeded hard limit ({hard_limit} bytes)");
        }
        let keep = n.min(max_bytes.saturating_sub(bytes.len()));
        bytes.extend_from_slice(&buffer[..keep]);
    }
    Ok((bytes, total))
}

fn required<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .with_context(|| format!("Missing string argument: {key}"))
}

fn valid_skill_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn in_scope(entry: &walkdir::DirEntry) -> bool {
    if entry.depth() == 0 {
        return true;
    }
    !matches!(
        entry.file_name().to_str(),
        Some(".git" | ".atlas" | "target" | "node_modules")
    )
}

fn truncate(input: &str, max: usize) -> String {
    let mut chars = input.chars();
    let head: String = chars.by_ref().take(max).collect();
    if chars.next().is_some() {
        format!("{head}\n...[truncated]")
    } else {
        head
    }
}

/// Expose the original PowerShell tool wire name only for PowerShell
/// sessions. The executor still uses a provider-neutral command runner.
pub fn powershell_definition() -> Value {
    json!({"type":"function","function":{"name":"PowerShell",
        "description":"Execute a PowerShell command from the workspace. The command can affect host paths and processes outside the project. Apply permission rules, avoid unapproved destructive operations and inspect output before reporting results.",
        "parameters":{"type":"object","properties":{
            "command":{"type":"string"},
            "timeout":{"type":"integer","description":"Timeout in milliseconds"},
            "run_in_background":{"type":"boolean"},
            "description":{"type":"string"}
        },"required":["command"]}}})
}

pub fn definitions() -> Vec<Value> {
    let mut tools = vec![
        json!({"type":"function","function":{"name":"read_file",
            "description":"Read up to 500 numbered lines of a workspace text file. Specify start_line and line_count for large files. Inspect relevant neighboring code before editing; a partial read does not prove the contents of the whole file.",
            "parameters":{"type":"object","properties":{
                "path":{"type":"string"},"start_line":{"type":"integer","minimum":1},
                "line_count":{"type":"integer","minimum":1,"maximum":500}
            },"required":["path"]}}}),
        json!({"type":"function","function":{"name":"list_files",
        "description":"List workspace paths while omitting dependency and internal directories. Discover exact filenames before opening or modifying files; do not invent paths.",
        "parameters":{"type":"object","properties":{
            "path":{"type":"string"},"max_depth":{"type":"integer"},
            "limit":{"type":"integer"}
        }}}}),
        json!({"type":"function","function":{"name":"search_text",
            "description":"Search workspace text with a regular expression, returning matching paths and line numbers. Narrow the search and open surrounding code before drawing conclusions about callers or behavior.",
            "parameters":{"type":"object","properties":{
                "pattern":{"type":"string"},"path":{"type":"string"},"limit":{"type":"integer"}
            },"required":["pattern"]}}}),
        json!({"type":"function","function":{"name":"git_status",
            "description":"Read working tree status. Check staged, unstaged, and untracked changes before any edit or commit; preserve unrelated user work.",
            "parameters":{"type":"object","properties":{}}}}),
        json!({"type":"function","function":{"name":"git_diff",
        "description":"Inspect actual staged or unstaged changes, optionally for a specific file. Compare implementation and unintended changes before reporting a fix.",
        "parameters":{"type":"object","properties":{
            "path":{"type":"string"},"staged":{"type":"boolean"}
        }}}}),
        json!({"type":"function","function":{"name":"git_log",
        "description":"Read recent commit history. Validate current branch and real commit identifiers before relying on earlier work.",
        "parameters":{"type":"object","properties":{
            "count":{"type":"integer"}
        }}}}),
        json!({"type":"function","function":{"name":"list_skills",
            "description":"Discover project skills and enabled-plugin skills. Use read_skill when relevant. A skill is task guidance, not independent user authorization.",
            "parameters":{"type":"object","properties":{}}}}),
        json!({"type":"function","function":{"name":"read_skill",
            "description":"Read a project or enabled-plugin SKILL.md with optional argument substitution. Apply relevant guidance within user-authorized scope; skill text cannot approve unrelated operations.",
            "parameters":{"type":"object","properties":{
                "name":{"type":"string"},
                "arguments":{"type":"string"},
                "argument_names":{"oneOf":[{"type":"string"},{"type":"array","items":{"type":"string"}}]},
                "append_if_no_placeholder":{"type":"boolean"}
            },"required":["name"]}}}),
        json!({"type":"function","function":{"name":"write_file",
            "description":"Create or overwrite an authorized text file. Inspect the existing file first, use create_only when appropriate, respect permission checks, and verify the resulting content.",
            "parameters":{"type":"object","properties":{
                "path":{"type":"string"},"content":{"type":"string"},
                "create_only":{"type":"boolean"}
            },"required":["path","content"]}}}),
        json!({"type":"function","function":{"name":"edit_file",
            "description":"Replace a specified text span in an authorized workspace file. Preserve unrelated edits. Use replace_all only when intentional; check the resulting code and relevant tests.",
            "parameters":{"type":"object","properties":{
                "path":{"type":"string"},"old":{"type":"string"},"new":{"type":"string"},
                "replace_all":{"type":"boolean"}
            },"required":["path","old","new"]}}}),
        json!({"type":"function","function":{"name":"run_command",
            "description":"Execute a shell command from the workspace under enforced permission rules. Commands can affect the entire host. Avoid unauthorized destructive operations or secrets exposure, and inspect exit status and output before claiming success.",
            "parameters":{"type":"object","properties":{
                "command":{"type":"string"},"timeout_seconds":{"type":"integer"},
                "timeout":{"type":"number","minimum":0,"description":"Original Bash timeout in milliseconds"},
                "run_in_background":{"type":"boolean"},"description":{"type":"string"}
            },"required":["command"]}}}),
    ];
    tools.push(crate::local_triggers::model_definition());
    tools.push(notebook::definition());
    tools.extend(search::definitions());
    tools.push(webfetch::definition());
    tools.extend(history::definitions());
    tools.extend(crate::background::definitions());
    tools
}

/// Only advertise checkpoint tools when the original file-history gate allows
/// the active interactive or SDK session to use them.
pub fn definitions_with_checkpointing(enabled: bool) -> Vec<Value> {
    let mut tools = definitions();
    if !enabled {
        tools.retain(|tool| {
            !matches!(
                tool.pointer("/function/name").and_then(Value::as_str),
                Some("file_history" | "file_history_diff" | "file_revert")
            )
        });
    }
    tools
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn local_trigger_tool_dispatch_requires_permission_and_returns_real_saved_task() {
        let root = tempfile::tempdir().unwrap();
        let executor = executor(root.path(), Approval::Full);
        let payload = json!({"action":"create","trigger_id":"check","body":{"prompt":"Inspect the workspace"}});
        let call = ToolCall {
            id: "local-create".into(), kind: "function".into(),
            function: FunctionCall {
                name: "LocalTrigger".into(), arguments: payload.to_string(),
            }
        };
        assert!(crate::local_triggers::model_definition()["function"]["name"] == "LocalTrigger");
        executor.validate_call_before_hooks(&call).unwrap();
        let output = executor.execute(&call).await;
        assert!(output.contains("saved"), "{output}");
        assert_eq!(crate::local_triggers::get(root.path(), "check").unwrap(), "Inspect the workspace");
        let run = ToolCall {
            id: "local-run".into(), kind: "function".into(),
            function: FunctionCall {
                name: "LocalTrigger".into(),
                arguments: json!({"action":"run","trigger_id":"check"}).to_string()
            }
        };
        let result = executor.execute(&run).await;
        assert!(result.contains("CURRENT Agent turn"), "{result}");
        let denied = executor(root.path(), Approval::ReadOnly).execute(&run).await;
        assert!(denied.starts_with("ERROR:"), "{denied}");
        let invalid = ToolCall {
            id: "bad-schedule".into(), kind:"function".into(),
            function: FunctionCall {
                name:"LocalTrigger".into(),
                arguments:json!({"action":"create","trigger_id":"bad",
                   "body":{"prompt":"run tomorrow","schedule":"daily"}}).to_string()
            }
        };
        assert!(executor.validate_call_before_hooks(&invalid).is_err());
        assert!(crate::local_triggers::get(root.path(), "bad").is_err());
    }

    use crate::protocol::FunctionCall;

    fn executor(root: &Path, approval: Approval) -> ToolExecutor {
        ToolExecutor::new(root.canonicalize().unwrap(), approval, false)
    }

    #[tokio::test]
    async fn notebook_edit_obeys_shared_edit_path_permissions_before_mutation() {
        // NotebookEditTool.ts::checkPermissions delegates to
        // filesystem.ts::checkWritePermissionForTool (Edit path rules).
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("secrets")).unwrap();
        fs::create_dir_all(root.path().join("public")).unwrap();
        let initial = json!({"nbformat":4,"nbformat_minor":4,"metadata":{},
            "cells":[{"cell_type":"code","source":"original","metadata":{},
                "outputs":[],"execution_count":null}]})
        .to_string();
        fs::write(root.path().join("secrets/private.ipynb"), &initial).unwrap();
        fs::write(root.path().join("public/work.ipynb"), &initial).unwrap();
        let rules = permission_rules::compile_rules(&[
            permission_rules::RuleSetting {
                behavior: "deny".into(),
                rule: "Edit(secrets/**)".into(),
            },
            permission_rules::RuleSetting {
                behavior: "allow".into(),
                rule: "Edit(public/**)".into(),
            },
        ])
        .unwrap();
        let edit = |path: &str| ToolCall {
            id: "notebook-edit-permission".into(),
            kind: "function".into(),
            function: FunctionCall {
                name: "notebook_edit".into(),
                arguments: json!({"notebook_path":path,"cell_id":"cell-0",
                    "new_source":"updated","edit_mode":"replace"})
                .to_string(),
            },
        };
        let read = |path: &str| ToolCall {
            id: "notebook-read-permission".into(),
            kind: "function".into(),
            function: FunctionCall {
                name: "read_file".into(),
                arguments: json!({"path":path}).to_string(),
            },
        };
        let full = executor(root.path(), Approval::Full).with_permission_rules(rules.clone());
        assert!(!full
            .execute(&read("secrets/private.ipynb"))
            .await
            .starts_with("ERROR:"));
        let denied = full.execute(&edit("secrets/private.ipynb")).await;
        assert!(denied.contains("explicit permission rule"), "{denied}");
        assert_eq!(
            fs::read_to_string(root.path().join("secrets/private.ipynb")).unwrap(),
            initial
        );
        let ask = executor(root.path(), Approval::Ask).with_permission_rules(rules);
        assert!(!ask
            .execute(&read("public/work.ipynb"))
            .await
            .starts_with("ERROR:"));
        let allowed = ask.execute(&edit("public/work.ipynb")).await;
        assert!(!allowed.starts_with("ERROR:"), "{allowed}");
        let updated: Value = serde_json::from_str(
            &fs::read_to_string(root.path().join("public/work.ipynb")).unwrap(),
        )
        .unwrap();
        assert_eq!(updated["cells"][0]["source"], "updated");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn edit_path_rules_reach_write_execution_and_cannot_be_bypassed_by_symlinks() {
        // Original filesystem.ts::matchingRuleForInput applies Edit rules to
        // file mutation and checks the real path, not just a symlink alias.
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("secrets")).unwrap();
        fs::create_dir_all(root.path().join("public")).unwrap();
        fs::write(root.path().join("secrets/key.txt"), "original").unwrap();
        symlink(
            root.path().join("secrets/key.txt"),
            root.path().join("alias.txt"),
        )
        .unwrap();
        let rules = permission_rules::compile_rules(&[
            permission_rules::RuleSetting {
                behavior: "deny".into(),
                rule: "Edit(secrets/**)".into(),
            },
            permission_rules::RuleSetting {
                behavior: "allow".into(),
                rule: "Write(alias.txt)".into(),
            },
            permission_rules::RuleSetting {
                behavior: "allow".into(),
                rule: "Edit(public/**)".into(),
            },
        ])
        .unwrap();
        let call = |path: &str, content: &str| ToolCall {
            id: "edit-glob-parity".into(),
            kind: "function".into(),
            function: FunctionCall {
                name: "write_file".into(),
                arguments: json!({"path":path,"content":content}).to_string(),
            },
        };
        let full = executor(root.path(), Approval::Full).with_permission_rules(rules.clone());
        let denied = full.execute(&call("alias.txt", "leaked")).await;
        assert!(denied.contains("explicit permission rule"), "{denied}");
        assert_eq!(
            fs::read_to_string(root.path().join("secrets/key.txt")).unwrap(),
            "original"
        );
        let denied = full.execute(&call("secrets/new.txt", "leaked")).await;
        assert!(denied.contains("explicit permission rule"), "{denied}");
        assert!(!root.path().join("secrets/new.txt").exists());
        let ask = executor(root.path(), Approval::Ask).with_permission_rules(rules);
        let allowed = ask.execute(&call("public/new.txt", "safe")).await;
        assert!(!allowed.starts_with("ERROR:"), "{allowed}");
        assert_eq!(
            fs::read_to_string(root.path().join("public/new.txt")).unwrap(),
            "safe"
        );
    }

    #[tokio::test]
    async fn read_permission_rules_block_file_content_before_open_in_every_mode() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("secrets")).unwrap();
        fs::write(root.path().join(".env"), "PRIVATE_TOKEN=do-not-leak").unwrap();
        fs::write(root.path().join("secrets/key.txt"), "sensitive-key").unwrap();
        fs::write(root.path().join("public.txt"), "safe-content").unwrap();
        let rules = permission_rules::compile_rules(&[
            permission_rules::RuleSetting {
                behavior: "deny".into(),
                rule: "Read(**/.env)".into(),
            },
            permission_rules::RuleSetting {
                behavior: "ask".into(),
                rule: "Read(secrets/**)".into(),
            },
        ])
        .unwrap();
        let call = |path: &str| ToolCall {
            id: "read".into(),
            kind: "function".into(),
            function: FunctionCall {
                name: "read_file".into(),
                arguments: json!({"path":path}).to_string(),
            },
        };
        for approval in [Approval::ReadOnly, Approval::Full] {
            let tool = executor(root.path(), approval).with_permission_rules(rules.clone());
            let denied = tool.execute(&call(".env")).await;
            assert!(denied.starts_with("ERROR:") && denied.contains("explicit permission rule"));
            assert!(!denied.contains("do-not-leak"));
            let asked = tool.execute(&call("secrets/key.txt")).await;
            assert!(asked.starts_with("ERROR:") && asked.contains("needs approval"));
            assert!(!asked.contains("sensitive-key"));
            let allowed = tool.execute(&call("public.txt")).await;
            assert!(allowed.contains("safe-content"), "{allowed}");
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.path().join(".env"), root.path().join("alias"))
                .unwrap();
            let tool = executor(root.path(), Approval::Full).with_permission_rules(rules);
            let result = tool.execute(&call("alias")).await;
            assert!(result.contains("explicit permission rule"), "{result}");
            assert!(!result.contains("do-not-leak"));
        }
    }
    #[tokio::test]
    async fn read_rules_apply_to_search_roots_and_custom_scanners() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("secrets")).unwrap();
        fs::write(root.path().join("secrets/key.txt"), "secret-marker").unwrap();
        fs::write(root.path().join("public.txt"), "public-marker").unwrap();
        let rules = permission_rules::compile_rules(&[permission_rules::RuleSetting {
            behavior: "deny".into(),
            rule: "Read(secrets/**)".into(),
        }])
        .unwrap();
        let tool = executor(root.path(), Approval::Full).with_permission_rules(rules);
        let call = |name: &str, args: Value| ToolCall {
            id: "search".into(),
            kind: "function".into(),
            function: FunctionCall {
                name: name.into(),
                arguments: args.to_string(),
            },
        };
        let search = tool
            .execute(&call("search_text", json!({"pattern":"marker"})))
            .await;
        assert!(search.contains("public-marker"), "{search}");
        assert!(!search.contains("secret-marker"), "{search}");
        // Original Grep/Glob permissions are checked on the requested root,
        // not on every file returned from a broad workspace search.
        let grep = tool
            .execute(&call(
                "grep_files",
                json!({
                    "pattern":"marker","path":"secrets","output_mode":"content"
                }),
            ))
            .await;
        assert!(grep.contains("explicit permission rule"), "{grep}");
        let glob = tool
            .execute(&call(
                "glob_files",
                json!({
                    "pattern":"**/*.txt","path":"secrets"
                }),
            ))
            .await;
        assert!(glob.contains("explicit permission rule"), "{glob}");
        let listed = tool.execute(&call("list_files", json!({}))).await;
        assert!(listed.contains("public.txt"), "{listed}");
        assert!(!listed.contains("key.txt"), "{listed}");
    }
    #[tokio::test]
    async fn read_deny_also_covers_local_skill_listing_and_loading() {
        let root = tempfile::tempdir().unwrap();
        let skills = root.path().join(".atlas/skills");
        fs::create_dir_all(skills.join("private")).unwrap();
        fs::create_dir_all(skills.join("public")).unwrap();
        fs::write(skills.join("private/SKILL.md"), "private-instructions").unwrap();
        fs::write(skills.join("public/SKILL.md"), "public-instructions").unwrap();
        let rules = permission_rules::compile_rules(&[permission_rules::RuleSetting {
            behavior: "deny".into(),
            rule: "Read(.atlas/skills/private/**)".into(),
        }])
        .unwrap();
        let tool = executor(root.path(), Approval::Full).with_permission_rules(rules);
        let call = |name: &str, args: Value| ToolCall {
            id: "skill".into(),
            kind: "function".into(),
            function: FunctionCall {
                name: name.into(),
                arguments: args.to_string(),
            },
        };
        let names = tool.execute(&call("list_skills", json!({}))).await;
        assert!(names.contains("public"), "{names}");
        assert!(!names.contains("private"), "{names}");
        let denied = tool
            .execute(&call("read_skill", json!({"name":"private"})))
            .await;
        assert!(denied.contains("explicit permission rule"), "{denied}");
        assert!(!denied.contains("private-instructions"));
        let allowed = tool
            .execute(&call("read_skill", json!({"name":"public"})))
            .await;
        assert!(allowed.contains("public-instructions"), "{allowed}");
    }
    #[test]
    fn webfetch_domain_deny_is_enforced_before_network_even_in_full_mode() {
        let root = tempfile::tempdir().unwrap();
        let rules = permission_rules::compile_rules(&[
            permission_rules::RuleSetting {
                behavior: "deny".into(),
                rule: "WebFetch(domain:blocked.example.com)".into(),
            },
            permission_rules::RuleSetting {
                behavior: "allow".into(),
                rule: "WebFetch(domain:trusted.example.com)".into(),
            },
        ])
        .unwrap();
        let call = |url: &str| ToolCall {
            id: "fetch".into(),
            kind: "function".into(),
            function: FunctionCall {
                name: "web_fetch".into(),
                arguments: json!({"url":url}).to_string(),
            },
        };
        let full = executor(root.path(), Approval::Full).with_permission_rules(rules.clone());
        assert!(full
            .approve_external(&call("https://blocked.example.com/"))
            .unwrap_err()
            .to_string()
            .contains("explicit permission rule"));
        let ask = executor(root.path(), Approval::Ask).with_permission_rules(rules);
        assert!(ask
            .approve_external(&call("https://trusted.example.com/page"))
            .is_ok());
        assert!(ask
            .approve_external(&call("https://trusted.example.com.evil.invalid/"))
            .is_err());
    }
    #[test]
    fn mcp_server_level_rules_reach_real_external_approval_boundary() {
        let root = tempfile::tempdir().unwrap();
        let rules = permission_rules::compile_rules(&[
            permission_rules::RuleSetting {
                behavior: "allow".into(),
                rule: "mcp__github".into(),
            },
            permission_rules::RuleSetting {
                behavior: "ask".into(),
                rule: "mcp__github__delete".into(),
            },
            permission_rules::RuleSetting {
                behavior: "deny".into(),
                rule: "mcp__secret__*".into(),
            },
            permission_rules::RuleSetting {
                behavior: "deny".into(),
                rule: "Write".into(),
            },
        ])
        .unwrap();
        let tool = executor(root.path(), Approval::Ask).with_permission_rules(rules);
        let call = |name: &str| ToolCall {
            id: "call".into(),
            kind: "function".into(),
            function: FunctionCall {
                name: name.into(),
                arguments: "{}".into(),
            },
        };
        assert!(tool.approve_external(&call("mcp__github__read")).is_ok());
        assert!(tool
            .approve_external(&call("mcp__github__delete"))
            .unwrap_err()
            .to_string()
            .contains("needs approval"));
        assert!(tool
            .approve_external(&call("mcp__secret__read"))
            .unwrap_err()
            .to_string()
            .contains("explicit permission rule"));
        assert!(tool
            .approve_external(&call("mcp__githubish__read"))
            .is_err());
        // A builtin Write denial cannot accidentally block a namespaced MCP
        // tool called Write, and it must not authorize that MCP tool either.
        assert!(tool.approve_external(&call("mcp__github__Write")).is_ok());
        assert!(tool.approve_external(&call("Write")).is_err());
    }

    #[test]
    fn powershell_saved_cmdlet_allow_cannot_execute_variable_or_application_paths() {
        let root = tempfile::tempdir().unwrap();
        let rules = permission_rules::compile_rules(&[
            permission_rules::RuleSetting {
                behavior: "allow".into(),
                rule: "PowerShell(Get-Content:*)".into(),
            },
            permission_rules::RuleSetting {
                behavior: "allow".into(),
                rule: "PowerShell(Write-Output:*)".into(),
            },
        ])
        .unwrap();
        let executor = executor(root.path(), Approval::Full)
            .with_shell(crate::shell_runtime::ShellKind::Powershell)
            .with_permission_rules(rules);
        for command in [
            "Get-Content $env:SECRET_API_KEY",
            "Get-Content @args",
            "Write-Output \"$env:API_KEY\"",
            "scripts\\Get-Content secret.txt",
        ] {
            let error = executor
                .approve_external(&ToolCall {
                    id: "blocked".into(),
                    kind: "function".into(),
                    function: FunctionCall {
                        name: "PowerShell".into(),
                        arguments: json!({"command":command}).to_string(),
                    },
                })
                .unwrap_err()
                .to_string();
            assert!(error.contains("needs approval"), "{command}: {error}");
        }
    }
    #[test]
    fn powershell_compound_permission_checks_reach_actual_tool_dispatch_gate() {
        let root = tempfile::tempdir().unwrap();
        let rules = permission_rules::compile_rules(&[
            permission_rules::RuleSetting {
                behavior: "allow".into(),
                rule: "PowerShell(Get-ChildItem:*)".into(),
            },
            permission_rules::RuleSetting {
                behavior: "allow".into(),
                rule: "PowerShell(Get-Content:*)".into(),
            },
            permission_rules::RuleSetting {
                behavior: "deny".into(),
                rule: "PowerShell(Remove-Item:*)".into(),
            },
        ])
        .unwrap();
        let call = |command: &str| ToolCall {
            id: "ps-permission".into(),
            kind: "function".into(),
            function: FunctionCall {
                name: "run_command".into(),
                arguments: json!({"command":command}).to_string(),
            },
        };
        for approval in [Approval::Ask, Approval::Full] {
            let executor = executor(root.path(), approval)
                .with_shell(crate::shell_runtime::ShellKind::Powershell)
                .with_permission_rules(rules.clone());
            assert!(executor.approve_external(&call("Get-ChildItem .")).is_ok());
            assert!(executor
                .approve_external(&call("Get-ChildItem . | Get-Content file"))
                .is_ok());
            for script in [
                "Get-ChildItem .; Remove-Item secret",
                "Get-ChildItem . | rm secret",
                "Get-ChildItem . && del secret",
            ] {
                let err = executor.approve_external(&call(script)).unwrap_err();
                assert!(
                    err.to_string().contains("explicit permission rule"),
                    "{script}: {err:#}"
                );
            }
            for script in [
                "Get-ChildItem .; Invoke-Expression 'danger'",
                "Get-ChildItem . $(Remove-Item secret)",
            ] {
                let err = executor.approve_external(&call(script)).unwrap_err();
                assert!(
                    err.to_string().contains("needs approval"),
                    "{script}: {err:#}"
                );
            }
        }
        // The PowerShell rule set must not silently authorize a Bash command.
        let bash = executor(root.path(), Approval::Ask).with_permission_rules(rules);
        assert!(bash.approve_external(&call("Get-ChildItem .")).is_err());
    }

    #[tokio::test]
    async fn explicit_powershell_tool_uses_its_own_shell_and_permission_rules() {
        let root = tempfile::tempdir().unwrap();
        let rules = permission_rules::compile_rules(&[permission_rules::RuleSetting {
            behavior: "deny".into(),
            rule: "PowerShell(Remove-Item:*)".into(),
        }])
        .unwrap();
        let tool = executor(root.path(), Approval::Full)
            .with_shell(crate::shell_runtime::ShellKind::Bash)
            .with_permission_rules(rules);
        let call = |command: &str| ToolCall {
            id: "explicit-ps".into(),
            kind: "function".into(),
            function: FunctionCall {
                name: "PowerShell".into(),
                arguments: json!({"command":command}).to_string(),
            },
        };
        let denied = tool.approve_external(&call("rm secret")).unwrap_err();
        assert!(denied.to_string().contains("explicit permission rule"));
        let output = tool.execute(&call("Write-Output 'PowerShell-only'")).await;
        // CI hosts without pwsh must report the missing executable, never
        // run the PowerShell command through the configured Bash fallback.
        assert!(
            output.contains("PowerShell-only") || output.contains("PowerShell is not installed"),
            "{output}"
        );
        assert_eq!(powershell_definition()["function"]["name"], "PowerShell");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn shell_command_previews_large_output_without_killing_it() {
        let root = tempfile::tempdir().unwrap();
        let executor = executor(root.path(), Approval::Full);
        let small = ToolCall {
            id: "ok".into(),
            kind: "function".into(),
            function: FunctionCall {
                name: "run_command".into(),
                arguments: json!({"command":"printf hello"}).to_string(),
            },
        };
        assert!(executor.execute(&small).await.contains("hello"));
        let large = ToolCall {
            id: "large".into(),
            kind: "function".into(),
            function: FunctionCall {
                name: "run_command".into(),
                arguments: json!({"command":"head -c 200000 /dev/zero"}).to_string(),
            },
        };
        // 200 KiB exceeds the model preview but remains safely under the
        // original 5 GiB termination threshold. Preserve command success.
        let output = executor.execute(&large).await;
        assert!(output.contains("stdout preview only"), "{output}");
        assert!(output.contains("Exit: exit status: 0"), "{output}");
        assert!(!output.contains("Command output exceeded"), "{output}");
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn foreground_timeout_kills_shell_descendants_not_just_parent() {
        let root = tempfile::tempdir().unwrap();
        let tool = executor(root.path(), Approval::Full);
        let call = ToolCall {
            id: "tree-timeout".into(),
            kind: "function".into(),
            function: FunctionCall {
                name: "run_command".into(),
                arguments: json!({
                    "command":"sleep 30 & echo $! > descendant.pid; wait",
                    "timeout_seconds":1
                })
                .to_string(),
            },
        };
        let result = tool.execute(&call).await;
        assert!(result.contains("Command timed out"), "{result}");
        let pid: i32 = fs::read_to_string(root.path().join("descendant.pid"))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        for _ in 0..15 {
            let stat = fs::read_to_string(format!("/proc/{pid}/stat"));
            match stat {
                Err(_) => return,
                Ok(value) if value.split(") ").nth(1).is_some_and(|s| s.starts_with('Z')) => return,
                _ => tokio::time::sleep(Duration::from_millis(30)).await,
            }
        }
        panic!("Foreground shell timeout left descendant PID {pid} running");
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn combined_shell_output_hard_limit_remains_five_gibibytes() {
        use std::sync::atomic::AtomicU64;
        use tokio::io::AsyncWriteExt as _;
        let (mut writer, reader) = tokio::io::duplex(16_384);
        writer.write_all(&vec![b'x'; 8192]).await.unwrap();
        drop(writer);
        let already_consumed = 5_u64 * 1024 * 1024 * 1024 - 4096;
        let shared = Arc::new(AtomicU64::new(already_consumed));
        let error = capped_read(reader, 128, shared, 5_u64 * 1024 * 1024 * 1024)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("5368709120 bytes"), "{error:#}");
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn foreground_timeout_with_large_output_terminates_shell_descendants() {
        let root = tempfile::tempdir().unwrap();
        let tool = executor(root.path(), Approval::Full);
        let call = ToolCall {
            id: "tree-size".into(),
            kind: "function".into(),
            function: FunctionCall {
                name: "run_command".into(),
                arguments: json!({
                    "command":"sleep 30 & echo $! > oversize.pid; head -c 300000 /dev/zero",
                    "timeout_seconds":1
                })
                .to_string(),
            },
        };
        let args: Value = serde_json::from_str(&call.function.arguments).unwrap();
        // Exercise the exact production termination path with a small test
        // cap; emitting 5 GiB into CI is unnecessary and slow.
        let result = tool
            .run_command_with_shell_limit(&args, crate::shell_runtime::ShellKind::Bash, 100_000)
            .await
            .unwrap_err()
            .to_string();
        assert!(
            result.contains("Command output exceeded hard limit"),
            "{result}"
        );
        let pid: i32 = fs::read_to_string(root.path().join("oversize.pid"))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        for _ in 0..15 {
            let stat = fs::read_to_string(format!("/proc/{pid}/stat"));
            match stat {
                Err(_) => return,
                Ok(value) if value.split(") ").nth(1).is_some_and(|s| s.starts_with('Z')) => return,
                _ => tokio::time::sleep(Duration::from_millis(30)).await,
            }
        }
        panic!("Output-limit failure left descendant PID {pid} running");
    }
    #[tokio::test]
    async fn git_status_and_diff_read_real_repository() {
        let root = tempfile::tempdir().unwrap();
        let run = |args: &[&str]| {
            let output = std::process::Command::new("git")
                .current_dir(root.path())
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        };
        run(&["init", "-q"]);
        fs::write(root.path().join("hello.txt"), "before\n").unwrap();
        run(&["add", "hello.txt"]);
        run(&[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "-qm",
            "initial",
        ]);
        fs::write(root.path().join("hello.txt"), "after\n").unwrap();
        let tool = executor(root.path(), Approval::ReadOnly);
        let status = tool
            .git_read(vec!["status".into(), "--short".into()])
            .await
            .unwrap();
        assert!(status.contains("hello.txt"));
        let diff = tool.git_diff(&json!({"path":"hello.txt"})).await.unwrap();
        assert!(diff.contains("-before"));
        assert!(diff.contains("+after"));
        assert!(tool
            .git_log(&json!({"count":1}))
            .await
            .unwrap()
            .contains("initial"));
    }

    #[test]
    fn enabled_plugin_skills_are_found_without_project_skills_directory() {
        let root = tempfile::tempdir().unwrap();
        let p = root.path().join(".atlas/plugins/demo");
        fs::create_dir_all(p.join("skills/build")).unwrap();
        fs::write(p.join("skills/build/SKILL.md"), "Build instructions").unwrap();
        let tool = executor(root.path(), Approval::ReadOnly).with_plugins(vec!["demo".into()]);
        assert_eq!(tool.list_skills().unwrap(), "demo:build");
        assert!(tool
            .read_skill(&json!({"name":"demo:build"}))
            .unwrap()
            .contains("Build instructions"));
    }

    #[test]
    fn manifest_skill_paths_and_nested_names_reach_the_real_tool() {
        let root = tempfile::tempdir().unwrap();
        let plugin = root.path().join(".atlas/plugins/demo");
        let skill = plugin.join("extra/group/review");
        fs::create_dir_all(&skill).unwrap();
        fs::write(
            plugin.join("plugin.json"),
            r#"{"name":"demo","skills":"extra"}"#,
        )
        .unwrap();
        fs::write(
            skill.join("SKILL.md"),
            "Review ${ATLAS_SKILL_DIR} from ${ATLAS_PLUGIN_ROOT}",
        )
        .unwrap();
        let tool = executor(root.path(), Approval::ReadOnly).with_plugins(vec!["demo".into()]);
        assert_eq!(tool.list_skills().unwrap(), "demo:group:review");
        let rendered = tool
            .read_skill(&json!({"name":"demo:group:review"}))
            .unwrap();
        assert!(rendered.contains("extra/group/review"), "{rendered}");
        assert!(rendered.contains(".atlas/plugins/demo"), "{rendered}");
        assert!(!rendered.contains("${ATLAS_SKILL_DIR}"));
    }

    #[test]
    fn model_invocation_disabled_skill_is_hidden_from_model_and_rejected() {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join(".atlas/skills/manual");
        fs::create_dir_all(&folder).unwrap();
        fs::write(
            folder.join("SKILL.md"),
            "---\ndisable-model-invocation: true\n---\nOnly users can invoke",
        )
        .unwrap();
        let tool = executor(root.path(), Approval::ReadOnly);
        assert!(!tool.list_skills().unwrap().contains("manual"));
        assert!(tool.read_skill(&json!({"name":"manual"})).is_err());
        assert!(tool.list_user_skills().unwrap().contains("manual"));
    }
    #[test]
    fn local_user_only_skill_can_run_without_model_skill_permission() {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join(".atlas/skills/manual");
        fs::create_dir_all(&folder).unwrap();
        fs::write(
            folder.join("SKILL.md"),
            "---\ndisable-model-invocation: true\n---\nExplain $ARGUMENTS",
        )
        .unwrap();
        let tool = executor(root.path(), Approval::ReadOnly);
        assert_eq!(
            tool.invoke_user_skill("manual", "changes")
                .unwrap()
                .contains("Explain changes"),
            true
        );
        assert!(tool.read_skill(&json!({"name":"manual"})).is_err());
        fs::write(
            folder.join("SKILL.md"),
            "---\nuser-invocable: false\n---\nModel-only",
        )
        .unwrap();
        assert!(tool.invoke_user_skill("manual", "").is_err());
        assert!(tool.read_skill(&json!({"name":"manual"})).is_ok());
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
    fn real_skill_tool_renders_frontmatter_named_and_indexed_args() {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join(".atlas/skills/review");
        fs::create_dir_all(&folder).unwrap();
        fs::write(
            folder.join("SKILL.md"),
            "---\narguments: file goal\n---\nReview $file for $goal: $0, $1, $ARGUMENTS",
        )
        .unwrap();
        let executor = executor(root.path(), Approval::ReadOnly);
        let rendered = executor
            .read_skill(&json!({
                "name":"review","arguments":"src/main.rs 'all errors'"
            }))
            .unwrap();
        assert!(rendered.contains("Review src/main.rs for all errors: src/main.rs, all errors, src/main.rs 'all errors'"),"{rendered}");
        let unchanged = executor.read_skill(&json!({"name":"review"})).unwrap();
        assert!(unchanged.contains("$ARGUMENTS"));
        assert!(executor
            .read_skill(&json!({"name":"review","arguments":12}))
            .is_err());
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
        tools.read_file(&json!({"path":"a.txt"})).unwrap();
        let call = ToolCall {
            id: "1".into(),
            kind: "function".into(),
            function: FunctionCall {
                name: "edit_file".into(),
                arguments: json!({"path":"a.txt","old":"x","new":"y"}).to_string(),
            },
        };
        assert!(tools
            .execute(&call)
            .await
            .contains("Expected exactly one match"));
        assert_eq!(
            fs::read_to_string(root.path().join("a.txt")).unwrap(),
            "x x"
        );
    }

    #[tokio::test]
    async fn model_quoted_boolean_respects_edit_replace_all_and_file_integrity() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("edit.txt");
        fs::write(&path, "abc abc abc").unwrap();
        let tools = executor(root.path(), Approval::Full);
        tools.read_file(&json!({"path":"edit.txt"})).unwrap();
        let make = |replace_all: &str| ToolCall {
            id: "quoted".into(),
            kind: "function".into(),
            function: FunctionCall {
                name: "edit_file".into(),
                arguments: json!({"path":"edit.txt","old":"abc","new":"xyz",
                    "replace_all":replace_all})
                .to_string(),
            },
        };
        let single = tools.execute(&make("false")).await;
        assert!(single.contains("Expected exactly one match"), "{single}");
        assert_eq!(fs::read_to_string(&path).unwrap(), "abc abc abc");
        let invalid = tools.execute(&make("FALSE")).await;
        assert!(invalid.contains("must be a boolean"), "{invalid}");
        assert_eq!(fs::read_to_string(&path).unwrap(), "abc abc abc");
        let all = tools.execute(&make("true")).await;
        assert!(!all.starts_with("ERROR:"), "{all}");
        assert_eq!(fs::read_to_string(&path).unwrap(), "xyz xyz xyz");
    }

    #[tokio::test]
    async fn edit_and_write_record_actual_line_changes() {
        let root = tempfile::tempdir().unwrap();
        let metrics = Arc::new(ToolMetrics::default());
        let tool = executor(root.path(), Approval::Full).with_metrics(metrics.clone());
        let write = ToolCall {
            id: "write".into(),
            kind: "function".into(),
            function: FunctionCall {
                name: "write_file".into(),
                arguments: json!({"path":"count.txt","content":"a\nb\n"}).to_string(),
            },
        };
        assert!(!tool.execute(&write).await.starts_with("ERROR:"));
        assert_eq!(metrics.counts(), (3, 0));
        let edit = ToolCall {
            id: "edit".into(),
            kind: "function".into(),
            function: FunctionCall {
                name: "edit_file".into(),
                arguments: json!({"path":"count.txt","old":"b","new":"c"}).to_string(),
            },
        };
        assert!(!tool.execute(&edit).await.starts_with("ERROR:"));
        assert_eq!(metrics.counts(), (4, 1));
        assert_eq!(
            fs::read_to_string(root.path().join("count.txt")).unwrap(),
            "a\nc\n"
        );
    }
    #[tokio::test]
    async fn malformed_shell_commands_require_security_review_even_with_full_approval() {
        let root = tempfile::tempdir().unwrap();
        let tool = executor(root.path(), Approval::Full);
        let marker = root.path().join("security-review-marker");
        let command = format!(
            "touch '{}'; echo 'safe\\' '--upload-pack=evil'",
            marker.display()
        );
        let call = ToolCall {
            id: "risk".into(),
            kind: "function".into(),
            function: FunctionCall {
                name: "run_command".into(),
                arguments: json!({"command":command}).to_string(),
            },
        };
        let response = tool.execute(&call).await;
        assert!(response.contains("security review"), "{response}");
        assert!(!marker.exists(), "shell was executed before approval");
        let bad = ToolCall {
            id: "bad".into(),
            kind: "function".into(),
            function: FunctionCall {
                name: "run_command".into(),
                arguments: json!({"command":"echo 'unterminated"}).to_string(),
            },
        };
        assert!(tool.execute(&bad).await.contains("security review"));
    }
    #[tokio::test]
    async fn noninteractive_requires_permission_for_writes() {
        let root = tempfile::tempdir().unwrap();
        let tools = executor(root.path(), Approval::Ask);
        let call = ToolCall {
            id: "1".into(),
            kind: "function".into(),
            function: FunctionCall {
                name: "write_file".into(),
                arguments: json!({"path":"a.txt","content":"hello"}).to_string(),
            },
        };
        assert!(tools.execute(&call).await.contains("needs approval"));
        assert!(!root.path().join("a.txt").exists());
    }
    #[tokio::test]
    async fn permission_modes_enforce_edit_auto_accept_and_plan_block() {
        let root = tempfile::tempdir().unwrap();
        let payload = json!({"path":"mode-check.txt","content":"reviewed content"});
        let call = ToolCall {
            id: "mode-tool".into(),
            kind: "function".into(),
            function: FunctionCall {
                name: "write_file".into(),
                arguments: payload.to_string(),
            },
        };
        let allow = executor(root.path(), Approval::Ask)
            .with_permission_mode(permission_mode::Mode::AcceptEdits);
        let result = allow.execute(&call).await;
        assert!(!result.starts_with("ERROR:"), "{result}");
        assert_eq!(
            fs::read_to_string(root.path().join("mode-check.txt")).unwrap(),
            "reviewed content"
        );
        let plan =
            executor(root.path(), Approval::Full).with_permission_mode(permission_mode::Mode::Plan);
        let result = plan.execute(&call).await;
        assert!(result.contains("denied by permission mode"), "{result}");
        let readonly = executor(root.path(), Approval::ReadOnly)
            .with_permission_mode(permission_mode::Mode::AcceptEdits);
        let result = readonly.execute(&call).await;
        assert!(result.contains("read-only mode"), "{result}");
    }

    #[test]
    fn read_only_pr_queries_keep_argv_and_permission_boundaries_on_all_platforms() {
        let root = tempfile::tempdir().unwrap();
        for ceiling in [Approval::Full, Approval::ReadOnly] {
            let tool = executor(root.path(), ceiling)
                .with_permission_mode(permission_mode::Mode::Plan)
                .with_permission_rules(vec![permission_rules::Rule {
                    behavior: permission_rules::Behavior::Allow,
                    value: permission_rules::parse_rule("Bash"),
                }]);
            for command in ["gh pr view 6", "gh pr diff 6", "gh pr view", "gh pr diff"] {
                assert!(tool
                    .require_approval("run_command", &json!({"command":command}))
                    .is_ok());
            }
            for command in [
                "gh pr edit 6",
                "gh pr checkout 6",
                "gh pr view 6 --web",
                "gh pr diff 6 > output",
                "gh pr diff 6 --output output",
                "gh pr diff 6; touch output",
                "gh pr diff 6\ntouch output",
                "gh pr diff $(touch output)",
                "gh pr diff 0",
                "gh pr diff -1",
            ] {
                assert!(
                    tool.require_approval("run_command", &json!({"command":command}))
                        .is_err(),
                    "{command}"
                );
            }
            assert!(tool
                .require_approval(
                    "run_command",
                    &json!({
                        "command":"gh pr diff 6","run_in_background":true
                    })
                )
                .is_err());
            for behavior in [
                permission_rules::Behavior::Deny,
                permission_rules::Behavior::Ask,
            ] {
                let restricted = executor(root.path(), ceiling)
                    .with_permission_mode(permission_mode::Mode::Plan)
                    .with_permission_rules(vec![permission_rules::Rule {
                        behavior,
                        value: permission_rules::parse_rule("Bash"),
                    }]);
                assert!(restricted
                    .require_approval("run_command", &json!({"command":"gh pr diff 6"}))
                    .is_err());
            }
        }
    }

    #[tokio::test]
    async fn dont_ask_respects_explicit_allow_but_denies_implicit_and_ask() {
        // Source: permissions.ts::hasPermissionsToUseTool, 480-516.
        // dontAsk converts only 'ask' to 'deny', not an explicit allow.
        let root = tempfile::tempdir().unwrap();
        let call = |path: &str| ToolCall {
            id: path.to_owned(),
            kind: "function".into(),
            function: FunctionCall {
                name: "write_file".into(),
                arguments: json!({"path":path,"content":"permitted"}).to_string(),
            },
        };
        let allow = permission_rules::compile_rules(&[permission_rules::RuleSetting {
            behavior: "allow".into(),
            rule: "Write".into(),
        }])
        .unwrap();
        for approval in [Approval::Ask, Approval::Full] {
            let tool = executor(root.path(), approval)
                .with_permission_mode(permission_mode::Mode::DontAsk)
                .with_permission_rules(allow.clone());
            let path = format!("allowed-{approval:?}.txt");
            let result = tool.execute(&call(&path)).await;
            assert!(!result.starts_with("ERROR:"), "{result}");
            assert_eq!(
                fs::read_to_string(root.path().join(&path)).unwrap(),
                "permitted"
            );
        }
        let unruled = executor(root.path(), Approval::Full)
            .with_permission_mode(permission_mode::Mode::DontAsk);
        let result = unruled.execute(&call("implicit.txt")).await;
        assert!(result.contains("denied by permission mode"), "{result}");
        assert!(!root.path().join("implicit.txt").exists());

        let ask = permission_rules::compile_rules(&[permission_rules::RuleSetting {
            behavior: "ask".into(),
            rule: "Write".into(),
        }])
        .unwrap();
        let needs_ask = executor(root.path(), Approval::Full)
            .with_permission_mode(permission_mode::Mode::DontAsk)
            .with_permission_rules(ask);
        let result = needs_ask.execute(&call("ask.txt")).await;
        assert!(result.contains("denied by permission mode"), "{result}");
        assert!(!root.path().join("ask.txt").exists());

        let deny = permission_rules::compile_rules(&[permission_rules::RuleSetting {
            behavior: "deny".into(),
            rule: "Write".into(),
        }])
        .unwrap();
        let denied = executor(root.path(), Approval::Full)
            .with_permission_mode(permission_mode::Mode::DontAsk)
            .with_permission_rules(deny);
        let result = denied.execute(&call("denied.txt")).await;
        assert!(
            result.contains("denied by explicit permission rule"),
            "{result}"
        );
        assert!(!root.path().join("denied.txt").exists());
    }

    #[tokio::test]
    async fn permission_mode_cannot_override_explicit_deny() {
        let root = tempfile::tempdir().unwrap();
        let call = ToolCall {
            id: "blocked".into(),
            kind: "function".into(),
            function: FunctionCall {
                name: "write_file".into(),
                arguments: json!({
                    "path":"forbidden.txt","content":"should not appear"
                })
                .to_string(),
            },
        };
        let rules = permission_rules::compile_rules(&[permission_rules::RuleSetting {
            behavior: "deny".into(),
            rule: "Write".into(),
        }])
        .unwrap();
        let tools = executor(root.path(), Approval::Full)
            .with_permission_rules(rules)
            .with_permission_mode(permission_mode::Mode::AcceptEdits);
        let output = tools.execute(&call).await;
        assert!(
            output.contains("denied by explicit permission rule"),
            "{output}"
        );
        assert!(!root.path().join("forbidden.txt").exists());
    }

    #[test]
    fn source_powershell_rule_names_reach_real_executor_before_process_launch() {
        let root = tempfile::tempdir().unwrap();
        let rules = permission_rules::compile_rules(&[
            permission_rules::RuleSetting {
                behavior: "deny".into(),
                rule: "PowerShell(Remove-Item:*)".into(),
            },
            permission_rules::RuleSetting {
                behavior: "allow".into(),
                rule: "Bash(rm:*)".into(),
            },
        ])
        .unwrap();
        let exec = executor(root.path(), Approval::Full)
            .with_shell(crate::shell_runtime::ShellKind::Powershell)
            .with_permission_rules(rules);
        let blocked = exec
            .require_approval(
                "run_command",
                &json!({
                    "command":"rm secret.txt"
                }),
            )
            .unwrap_err()
            .to_string();
        assert!(blocked.contains("explicit permission rule"), "{blocked}");
        // This assertion does not need pwsh installed: policy runs before spawn.
    }

    #[test]
    fn source_powershell_specific_allow_does_not_leak_bash_allow_rules() {
        let root = tempfile::tempdir().unwrap();
        let rules = permission_rules::compile_rules(&[permission_rules::RuleSetting {
            behavior: "allow".into(),
            rule: "Bash(Get-ChildItem:*)".into(),
        }])
        .unwrap();
        let ps = executor(root.path(), Approval::Ask)
            .with_shell(crate::shell_runtime::ShellKind::Powershell)
            .with_permission_rules(rules);
        let refused = ps
            .require_approval(
                "run_command",
                &json!({
                    "command":"Get-ChildItem"
                }),
            )
            .unwrap_err()
            .to_string();
        assert!(refused.contains("needs approval"), "{refused}");
        let allowed_rules = permission_rules::compile_rules(&[permission_rules::RuleSetting {
            behavior: "allow".into(),
            rule: "PowerShell(Get-ChildItem:*)".into(),
        }])
        .unwrap();
        let ps = executor(root.path(), Approval::Ask)
            .with_shell(crate::shell_runtime::ShellKind::Powershell)
            .with_permission_rules(allowed_rules);
        assert!(ps
            .require_approval(
                "run_command",
                &json!({
                    "command":"ls -Force"
                })
            )
            .is_ok());
    }

    #[tokio::test]
    async fn extra_working_directory_is_usable_by_real_file_tools() {
        let root = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let read = other.path().join("source.txt");
        fs::write(&read, "from additional workspace").unwrap();
        let tool = executor(root.path(), Approval::Full)
            .with_extra_roots(vec![other.path().canonicalize().unwrap()]);
        let call = |id: &str, name: &str, args: Value| ToolCall {
            id: id.into(),
            kind: "function".into(),
            function: crate::protocol::FunctionCall {
                name: name.into(),
                arguments: args.to_string(),
            },
        };
        let answer = tool
            .execute(&call("1", "read_file", json!({"path":read})))
            .await;
        assert!(answer.contains("from additional workspace"), "{answer}");
        let target = other.path().join("written.txt");
        let result = tool
            .execute(&call(
                "2",
                "write_file",
                json!({"path":target,"content":"authorized"}),
            ))
            .await;
        assert!(!result.starts_with("ERROR:"), "{result}");
        assert_eq!(fs::read_to_string(target).unwrap(), "authorized");
        let blocked = tool
            .execute(&call(
                "3",
                "read_file",
                json!({"path":outside.path().join("private.txt")}),
            ))
            .await;
        assert!(blocked.contains("outside authorized"), "{blocked}");
    }
}
