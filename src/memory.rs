use crate::memory_age;
use crate::{
    config::ProviderProfile,
    protocol::{Message, ModelRequest, TokenUsage},
    provider::ModelProvider,
};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    time::SystemTime,
};
use walkdir::WalkDir;

const MAX_INDEX_LINES: usize = 200;
const MAX_INDEX_BYTES: usize = 25_000;
const MAX_FILES: usize = 200;
const MAX_PER_TOPIC_BYTES: usize = 12_000;

#[derive(Clone, Debug)]
struct Header {
    path: PathBuf,
    filename: String,
    description: Option<String>,
    memory_type: Option<String>,
    modified: SystemTime,
}

pub struct MemoryContext {
    pub text: String,
    pub usage: Option<TokenUsage>,
}

fn trim_utf8(input: &str, cap: usize) -> String {
    if input.len() <= cap {
        return input.to_string();
    }
    let mut boundary = cap;
    while !input.is_char_boundary(boundary) {
        boundary -= 1;
    }
    input[..boundary].to_string()
}

fn index_content(content: &str) -> String {
    let trimmed = content.trim();
    let total_lines = trimmed.lines().count();
    let total_bytes = trimmed.len();
    let lines = trimmed
        .lines()
        .take(MAX_INDEX_LINES)
        .collect::<Vec<_>>()
        .join("\n");
    let capped = if lines.len() > MAX_INDEX_BYTES {
        let start = trim_utf8(&lines, MAX_INDEX_BYTES);
        let cut = start.rfind('\n').unwrap_or(start.len());
        start[..cut].to_string()
    } else {
        lines
    };
    if total_lines > MAX_INDEX_LINES || total_bytes > MAX_INDEX_BYTES {
        format!("{capped}\n\n> WARNING: MEMORY.md truncated ({} lines, {} bytes; limits {} lines/{} bytes).",
            total_lines, total_bytes, MAX_INDEX_LINES, MAX_INDEX_BYTES)
    } else {
        capped
    }
}

fn parse_header(text: &str) -> (Option<String>, Option<String>) {
    let mut description = None;
    let mut memory_type = None;
    let mut lines = text.lines();
    if lines.next() != Some("---") {
        return (description, memory_type);
    }
    for line in lines.take(29) {
        if line.trim() == "---" {
            break;
        }
        if let Some((key, value)) = line.split_once(':') {
            let val = value.trim().trim_matches(['"', '\'']).to_string();
            match key.trim() {
                "description" if !val.is_empty() => description = Some(val),
                "type" if matches!(val.as_str(), "user" | "feedback" | "project" | "reference") => {
                    memory_type = Some(val)
                }
                _ => {}
            }
        }
    }
    (description, memory_type)
}

fn scan(root: &Path) -> Result<(String, Vec<Header>)> {
    let dir = root.join(".atlas").join("memory");
    if !dir.exists() {
        return Ok((String::new(), Vec::new()));
    }
    let real = dir.canonicalize()?;
    let workspace = root.canonicalize()?;
    if !real.starts_with(&workspace) {
        bail!("Memory path leaves workspace")
    }
    let index_path = real.join("MEMORY.md");
    let index = if index_path.is_file() {
        if fs::metadata(&index_path)?.len() > 256_000 {
            bail!("MEMORY.md exceeds safety limit")
        }
        index_content(&fs::read_to_string(&index_path)?)
    } else {
        String::new()
    };
    let mut result = Vec::new();
    // The original memoryScan.ts scans recursively and settles each file
    // independently. A broken or unreadable topic must not hide healthy ones.
    for item in WalkDir::new(&real).follow_links(false) {
        let item = match item {
            Ok(item) => item,
            Err(_) => continue,
        };
        if !item.file_type().is_file()
            || item.path().extension().and_then(|e| e.to_str()) != Some("md")
            || item.file_name() == "MEMORY.md"
        {
            continue;
        }
        let path = match item.path().canonicalize() {
            Ok(path) if path.starts_with(&real) => path,
            _ => continue,
        };
        let meta = match fs::metadata(&path) {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        // Only the first 30 lines are needed for frontmatter. Avoid
        // excluding valid long topic files or reading their entire body.
        let file = match fs::File::open(&path) {
            Ok(file) => file,
            Err(_) => continue,
        };
        let header = match BufReader::new(file)
            .lines()
            .take(30)
            .collect::<std::io::Result<Vec<_>>>()
        {
            Ok(lines) => lines.join("\n"),
            Err(_) => continue,
        };
        let (description, memory_type) = parse_header(&header);
        let filename = match path.strip_prefix(&real) {
            Ok(name) => name.to_string_lossy().replace('\\', "/"),
            Err(_) => continue,
        };
        result.push(Header {
            path,
            filename,
            description,
            memory_type,
            modified: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
        });
    }
    result.sort_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then(a.filename.cmp(&b.filename))
    });
    result.truncate(MAX_FILES);
    Ok((index, result))
}

/// Select at most five memory files through the currently selected provider.
/// No service-specific secondary account or analytics request is made.
pub async fn context(
    root: &Path,
    query: &str,
    provider: &dyn ModelProvider,
    profile: &ProviderProfile,
) -> Result<MemoryContext> {
    context_with_recent_tools(root, query, provider, profile, &[]).await
}

/// The original memory selector receives recently used tools so that API
/// reference memories do not crowd out actionable warnings and gotchas.
pub async fn context_with_recent_tools(
    root: &Path,
    query: &str,
    provider: &dyn ModelProvider,
    profile: &ProviderProfile,
    recent_tools: &[String],
) -> Result<MemoryContext> {
    let (index, files) = scan(root)?;
    let mut text = String::new();
    if !index.is_empty() {
        text.push_str("Project memory index (local user-owned file; verify claims):\n");
        text.push_str(&index);
        text.push('\n');
    }
    if files.is_empty() {
        return Ok(MemoryContext { text, usage: None });
    }
    let manifest = files
        .iter()
        .map(|m| {
            let description = m.description.as_deref().unwrap_or("(no description)");
            let timestamp = chrono::DateTime::<chrono::Utc>::from(m.modified)
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
            let kind = m
                .memory_type
                .as_ref()
                .map(|kind| format!("[{kind}] "))
                .unwrap_or_default();
            if m.description.is_some() {
                format!("- {kind}{} ({timestamp}): {description}", m.filename)
            } else {
                format!("- {kind}{} ({timestamp})", m.filename)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let recent_tool_section = if recent_tools.is_empty() {
        String::new()
    } else {
        format!("\n\nRecently used tools: {}", recent_tools.join(", "))
    };
    let selection = provider
        .complete(&ModelRequest {
            model: profile.model.clone(),
            system: concat!(
                "Select up to five filenames of memories clearly relevant to the query. ",
                "Return only JSON: {\"selected_memories\":[\"filename.md\"]}. ",
                "If none are relevant return an empty array. Never invent filenames. ",
                "Do not select usage reference or API documentation for recently used tools; ",
                "do select their warnings, gotchas and known issues."
            )
            .into(),
            messages: vec![Message::text(
                "user",
                format!(
                    "Query:\n{query}\n\nMemory file manifest:\n{manifest}{recent_tool_section}"
                ),
            )],
            tools: vec![],
        })
        .await;
    // Original selectRelevantMemories is best-effort: a failed side query
    // returns no selections. Preserve MEMORY.md for the main agent even
    // when the provider is unavailable.
    let response = match selection {
        Ok(response) => response,
        Err(error) => {
            eprintln!("Local memory selection skipped: {error:#}");
            return Ok(MemoryContext { text, usage: None });
        }
    };
    if response
        .message
        .tool_calls
        .as_ref()
        .is_some_and(|calls| !calls.is_empty())
    {
        return Ok(MemoryContext {
            text,
            usage: response.usage,
        });
    }
    // sideQuery in 2.1.88 reads the first text block. Providers may
    // return structured SDK content instead of a flattened string.
    let selected = response
        .message
        .content
        .as_deref()
        .or_else(|| {
            response.message.content_blocks.as_ref().and_then(|blocks| {
                blocks.iter().find_map(|block| {
                    if block.get("type").and_then(Value::as_str) == Some("text") {
                        block.get("text").and_then(Value::as_str)
                    } else {
                        None
                    }
                })
            })
        })
        .unwrap_or("");
    let parsed: Value = match serde_json::from_str(selected) {
        Ok(value) => value,
        Err(_) => {
            return Ok(MemoryContext {
                text,
                usage: response.usage,
            })
        }
    };
    let mut seen = std::collections::HashSet::new();
    for name in parsed
        .get("selected_memories")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .take(50)
    {
        let Some(file) = files.iter().find(|f| f.filename == name) else {
            continue;
        };
        if !seen.insert(name) {
            continue;
        }
        if seen.len() > 5 {
            break;
        }
        // The topic may disappear or become unreadable after scanning.
        // Skip that selection and keep the other recalled memories.
        let contents = match fs::read_to_string(&file.path) {
            Ok(contents) => contents,
            Err(_) => continue,
        };
        let age_ms = match file.modified.duration_since(SystemTime::UNIX_EPOCH) {
            Ok(t) => t.as_secs_f64() * 1000.0,
            Err(t) => -(t.duration().as_secs_f64() * 1000.0),
        };
        let freshness = memory_age::freshness_note(age_ms);
        text.push_str(&format!(
            "\nRelevant local memory ({name}):\n{}{}\n",
            freshness,
            trim_utf8(&contents, MAX_PER_TOPIC_BYTES)
        ));
    }
    Ok(MemoryContext {
        text,
        usage: response.usage,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::ModelResponse;
    use anyhow::Result;
    use async_trait::async_trait;

    struct Selector;
    #[async_trait]
    impl ModelProvider for Selector {
        async fn complete(&self, req: &ModelRequest) -> Result<ModelResponse> {
            assert!(req.tools.is_empty());
            Ok(ModelResponse {
                message: Message::text(
                    "assistant",
                    r#"{"selected_memories":["topic.md","../../private.txt"]}"#,
                ),
                finish_reason: Some("stop".into()),
                usage: None,
            })
        }
    }
    fn profile() -> ProviderProfile {
        ProviderProfile {
            id: "local".into(),
            kind: "openai-compatible".into(),
            model: "mock".into(),
            base_url: "http://127.0.0.1:1/v1".into(),
            api_key_env: None,
            stream: false,
        }
    }
    #[test]
    fn respects_index_line_and_byte_caps() {
        let input = (0..250)
            .map(|i| format!("entry {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let capped = index_content(&input);
        assert!(capped.contains("entry 199"));
        assert!(!capped.contains("entry 200\n"));
        assert!(capped.contains("WARNING"));
        let huge = "文".repeat(9000);
        let capped = index_content(&huge);
        assert!(capped.contains("WARNING"));
        assert!(capped.is_char_boundary(capped.len()));
    }
    #[tokio::test]
    async fn selects_only_existing_local_memories() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let memory = root.join(".atlas/memory");
        fs::create_dir_all(&memory).unwrap();
        fs::write(memory.join("MEMORY.md"), "- [Topic](topic.md) index").unwrap();
        fs::write(
            memory.join("topic.md"),
            "---\ndescription: project choices\ntype: project\n---\nVerified choice",
        )
        .unwrap();
        fs::write(root.join("private.txt"), "SECRET").unwrap();
        let recalled = context(&root, "project choices", &Selector, &profile())
            .await
            .unwrap();
        assert!(recalled.text.contains("Verified choice"));
        assert!(!recalled.text.contains("SECRET"));
        assert!(recalled.text.contains("Topic"));
    }
    #[cfg(unix)]
    #[test]
    fn rejects_external_memory_directory_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".atlas")).unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join(".atlas/memory")).unwrap();
        assert!(scan(&dir.path().canonicalize().unwrap()).is_err());
    }
}
