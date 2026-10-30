//! Original commands/copy/copy.tsx: newest-first assistant response selection,
//! fenced-code selection, terminal clipboard and a durable local file fallback.
use crate::protocol::Message;
use anyhow::{bail, Context, Result};
use base64::Engine;
use std::{
    fs,
    io::{self, IsTerminal, Write},
    path::PathBuf,
};
use uuid::Uuid;

const MAX_LOOKBACK: usize = 20;

pub fn recent_assistant_texts(messages: &[Message]) -> Vec<String> {
    messages
        .iter()
        .rev()
        .filter(|message| message.role == "assistant")
        .map(|message| {
            // Original collectRecentAssistantTexts joins distinct text blocks
            // with two newlines and skips tool-use-only assistant turns.
            if let Some(blocks) = &message.content_blocks {
                let parts = blocks
                    .iter()
                    .filter_map(|block| {
                        if let Some(text) = block.as_str() {
                            return Some(text.to_owned());
                        }
                        if block.get("type").and_then(serde_json::Value::as_str) == Some("text") {
                            block
                                .get("text")
                                .and_then(serde_json::Value::as_str)
                                .map(str::to_owned)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                if !parts.is_empty() {
                    return parts.join("\n\n");
                }
                return String::new();
            }
            message.content.clone().unwrap_or_default()
        })
        .filter(|text| !text.trim().is_empty())
        .take(MAX_LOOKBACK)
        .collect()
}

#[derive(Debug, Clone)]
struct CodeBlock {
    code: String,
    language: String,
}

fn code_blocks(markdown: &str) -> Vec<CodeBlock> {
    // Marked's lexer recognizes both fenced and indented Markdown code.
    // The latter was missing from /copy --code, so users could only copy
    // fenced snippets even though the source picker listed both kinds.
    let mut cleaned = markdown.to_owned();
    for tag in [
        "commit_analysis",
        "context",
        "function_analysis",
        "pr_analysis",
    ] {
        let pattern = format!(r"(?s)<{tag}>.*?</{tag}>\\n?");
        let matcher = regex::Regex::new(&pattern).expect("fixed XML tag regex");
        cleaned = matcher.replace_all(&cleaned, "").into_owned();
    }
    let mut blocks = Vec::new();
    let mut fence: Option<(char, usize, String, Vec<String>)> = None;
    let mut indented: Option<Vec<String>> = None;
    let mut after_blank = true;
    for line in cleaned.lines() {
        let trimmed = line.trim_start_matches(' ');
        let indent = line.len() - trimmed.len();
        let marker = trimmed.chars().next().unwrap_or(' ');
        let run = trimmed.chars().take_while(|c| *c == marker).count();
        if let Some((open_marker, open_len, language, lines)) = fence.as_mut() {
            if marker == *open_marker && run >= *open_len
                && trimmed[run..].trim().is_empty() && indent <= 3
            {
                blocks.push(CodeBlock {
                    code: lines.join("\n"),
                    language: language.clone(),
                });
                fence = None;
            } else {
                lines.push(line.to_owned());
            }
            after_blank = false;
            continue;
        }
        if line.trim().is_empty() {
            if let Some(lines) = indented.as_mut() {
                lines.push(String::new());
            }
            after_blank = true;
            continue;
        }
        // CommonMark allows an indented block at document start or after a
        // blank line. Once started, additional indented lines continue it.
        let code_line = if line.starts_with('\t') {
            Some(&line[1..])
        } else if indent >= 4 {
            Some(&line[4..])
        } else {
            None
        };
        if let Some(content) = code_line {
            if after_blank || indented.is_some() {
                indented.get_or_insert_with(Vec::new).push(content.to_owned());
                after_blank = false;
                continue;
            }
        }
        if let Some(mut lines) = indented.take() {
            while lines.last().is_some_and(String::is_empty) {
                lines.pop();
            }
            blocks.push(CodeBlock { code: lines.join("\n"), language: String::new() });
        }
        if indent <= 3 && matches!(marker, '`' | '~') && run >= 3 {
            let rest = trimmed[run..].trim();
            if marker == '~' || !rest.contains('`') {
                let language = rest.split_whitespace().next().unwrap_or("");
                fence = Some((marker, run, language.to_owned(), Vec::new()));
                after_blank = false;
                continue;
            }
        }
        after_blank = false;
    }
    if let Some(mut lines) = indented {
        while lines.last().is_some_and(String::is_empty) {
            lines.pop();
        }
        blocks.push(CodeBlock { code: lines.join("\n"), language: String::new() });
    }
    // Marked also returns a code token for an unclosed fenced block.
    if let Some((_, _, language, lines)) = fence {
        blocks.push(CodeBlock { code: lines.join("\n"), language });
    }
    blocks
}

fn extension(language: &str) -> String {
    let safe = language
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>();
    if safe.is_empty() || safe == "plaintext" {
        "txt".into()
    } else {
        safe
    }
}


pub fn code_block_choices(messages: &[Message], age: usize) -> Result<Vec<(String, String)>> {
    let texts = recent_assistant_texts(messages);
    if age == 0 || age > texts.len() {
        bail!("No assistant response #{age}; {} recent responses available", texts.len());
    }
    Ok(code_blocks(&texts[age - 1]).into_iter().map(|block| {
        let preview = block.code.lines().next().unwrap_or("");
        let preview = preview.chars().take(76).collect::<String>();
        (if block.language.is_empty() { "text".into() } else { block.language }, preview)
    }).collect())
}

fn preference_path() -> Result<PathBuf> {
    let config = crate::config::Config::path()?;
    let parent = config.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    Ok(parent.join("copy-preferences.json"))
}
/// A terminal copy picker is optional, but "always copy full response" must
/// persist across sessions just like the original global copyFullResponse flag.
pub fn copy_full_preference() -> Result<bool> {
    let path = preference_path()?;
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    let value: serde_json::Value = serde_json::from_str(&raw)
        .context("Invalid copy preferences")?;
    value.get("copyFullResponse").and_then(serde_json::Value::as_bool)
        .context("copyFullResponse preference must be a boolean")
}
pub fn save_copy_full_preference(always: bool) -> Result<()> {
    let path = preference_path()?;
    let parent = path.parent().context("Copy preferences directory missing")?;
    fs::create_dir_all(parent)?;
    let staged = parent.join(format!(".copy-preferences-{}.tmp", Uuid::new_v4()));
    let result = (|| -> Result<()> {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&staged)?;
        file.write_all(serde_json::to_string_pretty(
            &serde_json::json!({"copyFullResponse":always})
        )?.as_bytes())?;
        file.sync_all()?;
        fs::rename(&staged, &path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&staged);
    }
    result
}

pub fn parse_selection(args: &str) -> Result<(usize, Option<usize>)> {
    let parts = args.split_whitespace().collect::<Vec<_>>();
    let (age, code) = match parts.as_slice() {
        [] => (1, None),
        ["--code", index] => (
            1,
            Some(
                index
                    .parse::<usize>()
                    .context("Invalid code block number")?,
            ),
        ),
        [age] => (
            age.parse::<usize>().context("Invalid response number")?,
            None,
        ),
        [age, "--code", index] => (
            age.parse::<usize>().context("Invalid response number")?,
            Some(
                index
                    .parse::<usize>()
                    .context("Invalid code block number")?,
            ),
        ),
        _ => bail!("Usage: /copy [N] [--code K]"),
    };
    if age == 0 || age > MAX_LOOKBACK || code == Some(0) {
        bail!("Copy selection indices must be positive (response N <= {MAX_LOOKBACK})");
    }
    Ok((age, code))
}

/// `/copy`, `/copy N`, `/copy N --code K` (N is 1-based newest first).
/// File output is mandatory even when the terminal accepts OSC 52.
pub fn copy(messages: &[Message], age: usize, code: Option<usize>) -> Result<String> {
    let texts = recent_assistant_texts(messages);
    if age == 0 || age > texts.len() {
        bail!(
            "No assistant response #{age}; {} recent responses available",
            texts.len()
        );
    }
    let selected = &texts[age - 1];
    let (text, name) = if let Some(index) = code {
        let blocks = code_blocks(selected);
        if index == 0 || index > blocks.len() {
            bail!(
                "No code block #{index}; {} code blocks in response #{age}",
                blocks.len()
            );
        }
        let block = &blocks[index - 1];
        (
            block.code.clone(),
            format!("copy.{}", extension(&block.language)),
        )
    } else {
        (selected.clone(), "response.md".to_owned())
    };

    // OSC 52 is a best-effort terminal clipboard operation in copy.tsx.
    // Attempt it before the fallback file: an unavailable temp directory
    // must not prevent copying text to a working interactive terminal.
    let mut clipboard_sent = false;
    if io::stdout().is_terminal() {
        let encoded = base64::engine::general_purpose::STANDARD.encode(text.as_bytes());
        let mut stdout = io::stdout().lock();
        stdout.write_all(format!("\u{1b}]52;c;{encoded}\u{7}").as_bytes())?;
        stdout.flush()?;
        clipboard_sent = true;
    }

    // A private, unpredictable directory prevents following a preexisting
    // symlink in the shared temporary location.
    let write_result = (|| -> Result<PathBuf> {
        let directory = std::env::temp_dir().join(format!("atlas-copy-{}", Uuid::new_v4()));
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = fs::DirBuilder::new();
            builder.mode(0o700);
            builder
                .create(&directory)
                .context("Create private copy output directory")?;
        }
        #[cfg(not(unix))]
        fs::create_dir(&directory).context("Create private copy output directory")?;
        let path = directory.join(name);
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        Ok(path)
    })();

    let char_count = text.encode_utf16().count();
    let line_count = text.bytes().filter(|b| *b == b'\n').count() + 1;
    match write_result {
        Ok(path) => {
            let clipboard = if clipboard_sent {
                "Clipboard sequence sent to terminal (terminal support varies)"
            } else {
                "No interactive clipboard; file fallback available"
            };
            Ok(format!(
                "{clipboard}. {char_count} characters, {line_count} lines. Saved: {}",
                path.display()
            ))
        }
        Err(error) if clipboard_sent => Ok(format!(
            "Clipboard sequence sent to terminal (terminal support varies). \
             {char_count} characters, {line_count} lines. \
             File fallback unavailable: {error}"
        )),
        Err(error) => {
            Err(error).context("Unable to copy: no interactive clipboard or file fallback")
        }
    }
}
