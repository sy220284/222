//! Prompt history and paste cache; separate from file checkpoint history.
use anyhow::{Context, Result};
use fs2::FileExt;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    time::SystemTime,
};

const MAX_ITEMS: usize = 100;
const MAX_INLINE_PASTE: usize = 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PastedContent {
    pub id: u64,
    #[serde(rename = "type")]
    pub kind: String,
    pub content: String,
    #[serde(default)]
    pub media_type: Option<String>,
    #[serde(default)]
    pub filename: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredPaste {
    id: u64,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    content_hash: Option<String>,
    #[serde(default)]
    media_type: Option<String>,
    #[serde(default)]
    filename: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LogEntry {
    display: String,
    #[serde(default)]
    pasted_contents: BTreeMap<u64, StoredPaste>,
    timestamp: i64,
    project: String,
    #[serde(default)]
    session_id: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryEntry {
    pub display: String,
    pub pasted_contents: BTreeMap<u64, PastedContent>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimestampedEntry {
    pub display: String,
    pub timestamp: i64,
}

pub struct PromptHistory {
    dir: PathBuf,
    project: String,
    session_id: String,
    pending: Vec<LogEntry>,
    last_added: Option<i64>,
    skipped: HashSet<i64>,
    last_timestamp: i64,
}
impl PromptHistory {
    pub fn new(workspace: &Path, session_id: &str) -> Result<Self> {
        let path = crate::config::Config::path()?;
        let dir = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        Self::with_dir(dir, workspace, session_id)
    }
    pub fn with_dir(dir: &Path, workspace: &Path, session_id: &str) -> Result<Self> {
        let project = workspace
            .canonicalize()
            .context("Prompt history workspace unavailable")?;
        Ok(Self {
            dir: dir.to_path_buf(),
            project: project.to_string_lossy().into_owned(),
            session_id: session_id.to_owned(),
            pending: Vec::new(),
            last_added: None,
            skipped: HashSet::new(),
            last_timestamp: 0,
        })
    }
    fn history_path(&self) -> PathBuf {
        self.dir.join("history.jsonl")
    }
    fn paste_dir(&self) -> PathBuf {
        self.dir.join("paste-cache")
    }

    pub fn add(&mut self, display: &str, pasted: BTreeMap<u64, PastedContent>) -> Result<()> {
        if std::env::var("ATLAS_SKIP_PROMPT_HISTORY")
            .ok()
            .as_deref()
            .is_some_and(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        {
            return Ok(());
        }
        let mut stored = BTreeMap::new();
        for (key, content) in pasted {
            if content.kind == "image" {
                continue;
            }
            let (inline, hash) = if content.content.encode_utf16().count() <= MAX_INLINE_PASTE {
                (Some(content.content), None)
            } else {
                let hash = paste_hash(&content.content);
                // The original stores large pasted text on a best-effort
                // path; a cache write failure must not discard the prompt.
                let _ = self.store_paste(&hash, &content.content);
                (None, Some(hash))
            };
            stored.insert(
                key,
                StoredPaste {
                    id: content.id,
                    kind: content.kind,
                    content: inline,
                    content_hash: hash,
                    media_type: content.media_type,
                    filename: content.filename,
                },
            );
        }
        let timestamp = chrono::Utc::now()
            .timestamp_millis()
            .max(self.last_timestamp.saturating_add(1));
        self.last_timestamp = timestamp;
        self.pending.push(LogEntry {
            display: display.to_owned(),
            pasted_contents: stored,
            timestamp,
            project: self.project.clone(),
            session_id: Some(self.session_id.clone()),
        });
        self.last_added = Some(timestamp);
        self.flush()
    }
    pub fn flush(&mut self) -> Result<()> {
        if self.pending.is_empty() {
            return Ok(());
        }
        fs::create_dir_all(&self.dir)?;
        let mut file = private_file(&self.history_path(), true)?;
        file.lock_exclusive()?;
        // Keep pending entries retryable without leaving a partial JSONL line
        // if an append or disk sync fails while holding the exclusive lock.
        let previous_len = file.metadata()?.len();
        let outcome = (|| -> Result<()> {
            let mut bytes = Vec::new();
            for entry in &self.pending {
                serde_json::to_writer(&mut bytes, entry)?;
                bytes.push(b'\n');
            }
            file.write_all(&bytes)?;
            file.sync_data()?;
            Ok(())
        })();
        if outcome.is_err() {
            let _ = file.set_len(previous_len);
        }
        let unlock = FileExt::unlock(&file);
        outcome?;
        unlock?;
        self.pending.clear();
        Ok(())
    }
    pub fn remove_last(&mut self) {
        let Some(timestamp) = self.last_added.take() else {
            return;
        };
        if let Some(index) = self.pending.iter().rposition(|e| e.timestamp == timestamp) {
            self.pending.remove(index);
        } else {
            self.skipped.insert(timestamp);
        }
    }
    pub fn clear_pending(&mut self) {
        self.pending.clear();
        self.last_added = None;
        self.skipped.clear();
    }
    /// Scan the global JSONL history newest-first without loading the entire
    /// file. The original history.ts uses readLinesReverse and stops after
    /// the first 100 current-project entries (or 100 distinct search results).
    fn newest_project_entries(&self, deduplicate: bool) -> Result<Vec<LogEntry>> {
        let mut entries = Vec::new();
        let mut seen = HashSet::new();
        let mut include = |entry: LogEntry| {
            if entry.project != self.project {
                return true;
            }
            if entry.session_id.as_deref() == Some(self.session_id.as_str())
                && self.skipped.contains(&entry.timestamp)
            {
                return true;
            }
            if deduplicate && !seen.insert(entry.display.clone()) {
                return true;
            }
            entries.push(entry);
            entries.len() < MAX_ITEMS
        };
        for entry in self.pending.iter().rev() {
            if !include(entry.clone()) {
                return Ok(entries);
            }
        }
        let mut file = match File::open(self.history_path()) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(entries);
            }
            Err(error) => return Err(error.into()),
        };
        let mut position = file.metadata()?.len();
        let mut reversed_line = Vec::new();
        while position > 0 {
            let chunk_size = position.min(64 * 1024) as usize;
            position -= chunk_size as u64;
            file.seek(SeekFrom::Start(position))?;
            let mut chunk = vec![0; chunk_size];
            file.read_exact(&mut chunk)?;
            for byte in chunk.into_iter().rev() {
                if byte == b'\n' {
                    if reversed_line.is_empty() {
                        continue;
                    }
                    reversed_line.reverse();
                    let keep_scanning = serde_json::from_slice::<LogEntry>(&reversed_line)
                        .map(|entry| include(entry))
                        .unwrap_or(true);
                    reversed_line.clear();
                    if !keep_scanning {
                        return Ok(entries);
                    }
                } else {
                    reversed_line.push(byte);
                }
            }
        }
        if !reversed_line.is_empty() {
            reversed_line.reverse();
            if let Ok(entry) = serde_json::from_slice::<LogEntry>(&reversed_line) {
                include(entry);
            }
        }
        Ok(entries)
    }
    fn resolve(&self, entry: &LogEntry) -> HistoryEntry {
        let mut pasted_contents = BTreeMap::new();
        for (key, paste) in &entry.pasted_contents {
            let content = paste
                .content
                .as_ref()
                .filter(|v| !v.is_empty())
                .cloned()
                .or_else(|| {
                    paste
                        .content_hash
                        .as_deref()
                        .and_then(|hash| self.retrieve_paste(hash))
                });
            if let Some(content) = content {
                pasted_contents.insert(
                    *key,
                    PastedContent {
                        id: paste.id,
                        kind: paste.kind.clone(),
                        content,
                        media_type: paste.media_type.clone(),
                        filename: paste.filename.clone(),
                    },
                );
            }
        }
        HistoryEntry {
            display: entry.display.clone(),
            pasted_contents,
        }
    }
    pub fn get_history(&self) -> Result<Vec<HistoryEntry>> {
        let mut current = Vec::new();
        let mut others = Vec::new();
        for entry in self.newest_project_entries(false)? {
            if entry.session_id.as_deref() == Some(self.session_id.as_str()) {
                current.push(entry);
            } else {
                others.push(entry);
            }
        }
        current.extend(others);
        Ok(current.iter().map(|entry| self.resolve(entry)).collect())
    }
    pub fn get_timestamped_history(&self) -> Result<Vec<TimestampedEntry>> {
        Ok(self.newest_project_entries(true)?
            .into_iter()
            .map(|entry| TimestampedEntry {
                display: entry.display,
                timestamp: entry.timestamp,
            })
            .collect())
    }
    fn store_paste(&self, hash: &str, content: &str) -> Result<()> {
        fs::create_dir_all(self.paste_dir())?;
        let path = self.paste_dir().join(format!("{hash}.txt"));
        // Readers in other sessions must never observe a half-written paste.
        let temp = self.paste_dir().join(format!(".{hash}-{}.tmp", uuid::Uuid::new_v4()));
        let outcome = (|| -> Result<()> {
            let mut file = private_file(&temp, false)?;
            file.write_all(content.as_bytes())?;
            file.sync_all()?;
            fs::rename(&temp, &path)?;
            Ok(())
        })();
        if outcome.is_err() {
            let _ = fs::remove_file(&temp);
        }
        outcome
    }
    fn retrieve_paste(&self, hash: &str) -> Option<String> {
        if hash.len() != 16 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        fs::read_to_string(self.paste_dir().join(format!("{hash}.txt"))).ok()
    }
    pub fn cleanup_old_pastes(&self, cutoff: SystemTime) -> Result<usize> {
        let mut removed = 0;
        let dir = match fs::read_dir(self.paste_dir()) {
            Ok(dir) => dir,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(e) => return Err(e.into()),
        };
        for entry in dir {
            let entry = entry?;
            if entry.path().extension().and_then(|e| e.to_str()) != Some("txt") {
                continue;
            }
            if entry
                .metadata()
                .and_then(|m| m.modified())
                .is_ok_and(|time| time < cutoff)
                && fs::remove_file(entry.path()).is_ok()
            {
                removed += 1
            }
        }
        Ok(removed)
    }
}
pub fn paste_hash(content: &str) -> String {
    format!("{:x}", Sha256::digest(content.as_bytes()))[..16].to_owned()
}
fn private_file(path: &Path, append: bool) -> Result<File> {
    let mut options = OpenOptions::new();
    options.create(true).write(true);
    if append {
        options.append(true).read(true);
    } else {
        options.truncate(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    Ok(options.open(path)?)
}
pub fn format_pasted_text_ref(id: u64, text: &str) -> String {
    let lines = text
        .replace("\r\n", "\n")
        .chars()
        .filter(|c| *c == '\n' || *c == '\r')
        .count();
    if lines == 0 {
        format!("[Pasted text #{id}]")
    } else {
        format!("[Pasted text #{id} +{lines} lines]")
    }
}
pub fn format_image_ref(id: u64) -> String {
    format!("[Image #{id}]")
}
pub fn expand_pasted_text_refs(input: &str, pasted: &BTreeMap<u64, PastedContent>) -> String {
    let regex =
        Regex::new(r"\[(?:Pasted text|Image|\.\.\.Truncated text) #(\d+)(?: \+\d+ lines)?\.*\]")
            .unwrap();
    let matches = regex
        .captures_iter(input)
        .filter_map(|capture| {
            let whole = capture.get(0)?;
            let id = capture
                .get(1)?
                .as_str()
                .parse::<u64>()
                .ok()
                .filter(|id| *id > 0)?;
            Some((whole.start(), whole.end(), id))
        })
        .collect::<Vec<_>>();
    let mut expanded = input.to_owned();
    for (start, end, id) in matches.into_iter().rev() {
        if let Some(paste) = pasted.get(&id).filter(|p| p.kind == "text") {
            expanded.replace_range(start..end, &paste.content);
        }
    }
    expanded
}
#[cfg(test)]
mod tests {
    use super::*;
    fn make(dir: &Path, project: &Path, session: &str) -> PromptHistory {
        PromptHistory::with_dir(dir, project, session).unwrap()
    }
    #[test]
    fn current_session_priority_search_dedupe_and_undo() {
        let root = tempfile::tempdir().unwrap();
        let project = root.path().join("p");
        fs::create_dir_all(&project).unwrap();
        let other = root.path().join("other");
        fs::create_dir_all(&other).unwrap();
        let config = root.path().join("config");
        make(&config, &project, "other")
            .add("old", BTreeMap::new())
            .unwrap();
        let mut current = make(&config, &project, "current");
        current.add("new", BTreeMap::new()).unwrap();
        make(&config, &other, "current")
            .add("hidden", BTreeMap::new())
            .unwrap();
        current.add("new", BTreeMap::new()).unwrap();
        assert_eq!(
            current
                .get_history()
                .unwrap()
                .iter()
                .map(|e| e.display.as_str())
                .collect::<Vec<_>>(),
            vec!["new", "new", "old"]
        );
        assert_eq!(current.get_timestamped_history().unwrap().len(), 2);
        current.remove_last();
        assert_eq!(current.get_history().unwrap().len(), 2);
        current.remove_last();
        assert_eq!(current.get_history().unwrap().len(), 2);
        assert_eq!(
            make(&config, &project, "restarted")
                .get_history()
                .unwrap()
                .len(),
            3
        );
    }
    #[test]
    fn paste_roundtrip_and_reference_offsets() {
        let root = tempfile::tempdir().unwrap();
        let mut history = make(root.path(), root.path(), "session");
        let large = format!("[Pasted text #2]{}", "x".repeat(1100));
        let mut pastes = BTreeMap::new();
        pastes.insert(
            1,
            PastedContent {
                id: 1,
                kind: "text".into(),
                content: large.clone(),
                media_type: None,
                filename: None,
            },
        );
        pastes.insert(
            2,
            PastedContent {
                id: 2,
                kind: "image".into(),
                content: "image".into(),
                media_type: None,
                filename: None,
            },
        );
        history.add("[Pasted text #1] [Image #2]", pastes).unwrap();
        let entries = history.get_history().unwrap();
        let entry = &entries[0];
        assert_eq!(entry.pasted_contents.len(), 1);
        assert_eq!(entry.pasted_contents[&1].content, large);
        assert_eq!(
            expand_pasted_text_refs(&entry.display, &entry.pasted_contents),
            format!("{large} [Image #2]")
        );
        assert_eq!(
            format_pasted_text_ref(1, "a\r\nb\nc\rd"),
            "[Pasted text #1 +3 lines]"
        );
        assert_eq!(format_image_ref(2), "[Image #2]");
    }
    #[test]
    fn concurrent_writes_preserve_json_lines_and_skip_corruption() {
        let root = tempfile::tempdir().unwrap();
        let config = root.path().join("config");
        let mut handles = Vec::new();
        for worker in 0..4 {
            let config = config.clone();
            let project = root.path().to_path_buf();
            handles.push(std::thread::spawn(move || {
                let mut history = make(&config, &project, &format!("worker-{worker}"));
                for index in 0..20 {
                    history
                        .add(&format!("{worker}-{index}"), BTreeMap::new())
                        .unwrap();
                }
            }));
        }
        for handle in handles {
            handle.join().unwrap()
        }
        let mut file = private_file(&config.join("history.jsonl"), true).unwrap();
        writeln!(file, "{{invalid-json").unwrap();
        assert_eq!(
            make(&config, root.path(), "reader")
                .get_timestamped_history()
                .unwrap()
                .len(),
            80
        );
    }
}
