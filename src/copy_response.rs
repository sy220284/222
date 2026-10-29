//! Original commands/copy/copy.tsx: newest-first assistant response selection,
//! fenced-code selection, terminal clipboard and a durable local file fallback.
use crate::protocol::Message;
use anyhow::{bail, Context, Result};
use base64::Engine;
use std::{fs, io::{self, IsTerminal, Write}, path::PathBuf};
use uuid::Uuid;

const MAX_LOOKBACK: usize = 20;

pub fn recent_assistant_texts(messages: &[Message]) -> Vec<String> {
    messages.iter().rev()
        .filter(|message| message.role == "assistant")
        .map(Message::visible_text)
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
    // Original copy.tsx strips hidden prompt analysis XML before Markdown
    // lexing. Keep the full response untouched when copying it directly.
    let mut cleaned = markdown.to_owned();
    for tag in ["commit_analysis", "context", "function_analysis", "pr_analysis"] {
        let pattern = format!(r"(?s)<{tag}>.*?</{tag}>\n?");
        let matcher = regex::Regex::new(&pattern).expect("fixed XML tag regex");
        cleaned = matcher.replace_all(&cleaned, "").into_owned();
    }
    let mut blocks = Vec::new();
    let mut fence: Option<(char, usize, String, Vec<String>)> = None;
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
        } else if indent <= 3 && matches!(marker, '`' | '~') && run >= 3 {
            let rest = trimmed[run..].trim();
            if marker == '~' || !rest.contains('`') {
                let language = rest.split_whitespace().next().unwrap_or("");
                fence = Some((marker, run, language.to_owned(), Vec::new()));
            }
        }
    }
    blocks
}

fn extension(language: &str) -> String {
    let safe = language.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>();
    if safe.is_empty() || safe == "plaintext" {
        "txt".into()
    } else {
        safe
    }
}

pub fn parse_selection(args: &str) -> Result<(usize, Option<usize>)> {
    let parts = args.split_whitespace().collect::<Vec<_>>();
    let (age, code) = match parts.as_slice() {
        [] => (1, None),
        ["--code", index] => (1, Some(index.parse::<usize>().context("Invalid code block number")?)),
        [age] => (age.parse::<usize>().context("Invalid response number")?, None),
        [age, "--code", index] => (
            age.parse::<usize>().context("Invalid response number")?,
            Some(index.parse::<usize>().context("Invalid code block number")?),
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
        bail!("No assistant response #{age}; {} recent responses available", texts.len());
    }
    let selected = &texts[age - 1];
    let (text, name) = if let Some(index) = code {
        let blocks = code_blocks(selected);
        if index == 0 || index > blocks.len() {
            bail!("No code block #{index}; {} code blocks in response #{age}", blocks.len());
        }
        let block = &blocks[index - 1];
        (block.code.clone(), format!("copy.{}", extension(&block.language)))
    } else {
        (selected.clone(), "response.md".to_owned())
    };

    // A new, unpredictable directory prevents following a preexisting
    // symlink in a shared temporary location.
    let directory = std::env::temp_dir().join(format!("atlas-copy-{}", Uuid::new_v4()));
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        let mut builder = fs::DirBuilder::new();
        builder.mode(0o700);
        builder.create(&directory).context("Create private copy output directory")?;
    }
    #[cfg(not(unix))]
    fs::create_dir(&directory).context("Create private copy output directory")?;
    let path: PathBuf = directory.join(name);
    let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&path)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;

    // OSC 52 may be disabled by a terminal; never treat it as confirmed.
    let clipboard = if io::stdout().is_terminal() {
        let encoded = base64::engine::general_purpose::STANDARD.encode(text.as_bytes());
        let mut stdout = io::stdout().lock();
        stdout.write_all(format!("\u{1b}]52;c;{encoded}\u{7}").as_bytes())?;
        stdout.flush()?;
        "Clipboard sequence sent to terminal (terminal support varies)"
    } else {
        "No interactive clipboard; file fallback available"
    };
    Ok(format!("{clipboard}. {} characters, {} lines. Saved: {}",
        text.chars().count(), text.lines().count().max(1), path.display()))
}
