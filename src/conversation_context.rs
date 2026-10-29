//! Conversation-scoped user context from the 2.1.88 context.ts contract.
//! The Git snapshot is taken once before the first model request, rather than
//! silently changing the model's initial repository view on later turns.
use chrono::Local;
use std::{path::Path, process::Command};

const MAX_STATUS_CHARS: usize = 2_000;

#[derive(Debug, Clone)]
pub struct ConversationContext {
    date: String,
    git_status: Option<String>,
}

impl ConversationContext {
    /// Current Git branch at session creation, independent of the optional
    /// context shown to the model. Detached HEAD returns None.
    pub fn branch_at_start(root: &Path) -> Option<String> {
        branch_at_start(root)
    }

    pub fn capture(root: &Path) -> Self {
        let git_status = if git_context_enabled() {
            git_status_at_start(root)
        } else {
            None
        };
        Self {
            date: Local::now().format("%Y-%m-%d").to_string(),
            git_status,
        }
    }

    pub fn append_to(&self, prompt: &mut String) {
        if let Some(git) = &self.git_status {
            prompt.push_str("\nRepository context captured at the start of this conversation:\n");
            prompt.push_str(git);
            prompt.push('\n');
        }
        prompt.push_str("\nToday's date is ");
        prompt.push_str(&self.date);
        prompt.push_str(".\n");
    }
}

fn git_context_enabled() -> bool {
    // Vendor-specific remote flags are intentionally not retained.
    !matches!(
        std::env::var("ATLAS_DISABLE_GIT_INSTRUCTIONS")
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("--no-optional-locks")
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8(output.stdout).ok()?.trim().to_owned())
}

/// Record a stable branch label for session discovery, independent of the
/// optional Git instructions sent to the model.
pub fn branch_at_start(root: &Path) -> Option<String> {
    if git(root, &["rev-parse", "--is-inside-work-tree"])?.trim() != "true" {
        return None;
    }
    git(root, &["symbolic-ref", "--quiet", "--short", "HEAD"]).filter(|branch| !branch.is_empty())
}

fn git_status_at_start(root: &Path) -> Option<String> {
    if git(root, &["rev-parse", "--is-inside-work-tree"])?.trim() != "true" {
        return None;
    }
    let branch = git(root, &["symbolic-ref", "--quiet", "--short", "HEAD"])
        .unwrap_or_else(|| "(detached HEAD)".into());
    let main_branch = git(
        root,
        &[
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ],
    )
    .and_then(|value| value.strip_prefix("origin/").map(str::to_owned))
    .or_else(|| {
        ["main", "master"]
            .into_iter()
            .find(|name| {
                git(
                    root,
                    &[
                        "show-ref",
                        "--verify",
                        "--quiet",
                        &format!("refs/heads/{name}"),
                    ],
                )
                .is_some()
            })
            .map(str::to_owned)
    })
    .unwrap_or_else(|| "(unknown)".into());
    let status = git(root, &["status", "--short"]).unwrap_or_default();
    let log = git(root, &["log", "--oneline", "-n", "5"]).unwrap_or_default();
    let user_name = git(root, &["config", "user.name"]).unwrap_or_default();
    // Original context.ts uses JavaScript string.length and substring:
    // UTF-16 code units, including astral Unicode in Git filenames.
    let utf16_len = status.encode_utf16().count();
    let shortened = if utf16_len > MAX_STATUS_CHARS {
        let mut used = 0;
        let mut prefix = String::new();
        for ch in status.chars() {
            if used + ch.len_utf16() > MAX_STATUS_CHARS {
                break;
            }
            prefix.push(ch);
            used += ch.len_utf16();
        }
        format!(
            "{prefix}\n... (truncated because it exceeds 2k characters. Run git status for more information)"
        )
    } else {
        status
    };
    Some(format!(
        "This is the git status at the start of the conversation. Note that this status is a snapshot in time, and will not update during the conversation.\n\nCurrent branch: {branch}\n\nMain branch (you will usually use this for PRs): {main_branch}\n\n{}Status:\n{}\n\nRecent commits:\n{log}",
        if user_name.is_empty() { String::new() } else { format!("Git user: {user_name}\n\n") },
        if shortened.is_empty() { "(clean)" } else { &shortened }
    ))
}

/// Load user and project instructions in the outer-to-inner order of the
/// original project-context and instruction-discovery modules, using AGENTS.md and
/// .atlas paths. File-conditional rules never enter the global system prompt.
pub fn project_instructions(root: &Path) -> anyhow::Result<String> {
    use anyhow::{bail, Context};
    use std::{collections::HashSet, fs, path::PathBuf};

    const MAX_FILE_BYTES: u64 = 50_000;
    const MAX_FILES: usize = 200;
    const MAX_RULE_DEPTH: usize = 8;

    fn enabled() -> bool {
        !std::env::var("ATLAS_DISABLE_PROJECT_INSTRUCTIONS")
            .ok()
            .is_some_and(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
    }

    fn rule_body(content: &str) -> Option<&str> {
        let normalized = content
            .strip_prefix("---\r\n")
            .or_else(|| content.strip_prefix("---\n"));
        let Some(rest) = normalized else {
            return Some(content);
        };
        let mut header_len = 0;
        let mut conditional = false;
        let mut ended = false;
        for line in rest.split_inclusive('\n') {
            header_len += line.len();
            let trimmed = line.trim();
            if trimmed == "---" {
                ended = true;
                break;
            }
            if trimmed.starts_with("paths:") {
                conditional = true;
            }
        }
        if !ended {
            return Some(content);
        }
        if conditional {
            None
        } else {
            Some(&rest[header_len..])
        }
    }

    // 2.1.88 claudemd.ts extracts @includes from Markdown text nodes, skips
    // fenced/inline code and HTML comments, and limits recursive expansion.
    // An unapproved include outside the current instruction scope is ignored.
    fn include_paths(content: &str, file: &Path) -> Vec<PathBuf> {
        let mut plain = String::new();
        let mut fence: Option<char> = None;
        let mut html_comment = false;
        for line in content.lines() {
            let trimmed = line.trim_start();
            if let Some(marker) = trimmed.chars().next().filter(|c| *c == '`' || *c == '~') {
                if trimmed.chars().take_while(|c| *c == marker).count() >= 3 {
                    if fence == Some(marker) {
                        fence = None;
                    } else if fence.is_none() {
                        fence = Some(marker);
                    }
                    continue;
                }
            }
            if fence.is_some() {
                continue;
            }
            let mut inline_code = false;
            let mut cursor = 0;
            while cursor < line.len() {
                let remaining = &line[cursor..];
                if html_comment {
                    if let Some(end) = remaining.find("-->") {
                        cursor += end + 3;
                        html_comment = false;
                    } else {
                        break;
                    }
                    continue;
                }
                if remaining.starts_with("<!--") {
                    html_comment = true;
                    cursor += 4;
                    continue;
                }
                let ch = remaining.chars().next().expect("cursor is inside line");
                if ch == '`' {
                    inline_code = !inline_code;
                } else if !inline_code {
                    plain.push(ch);
                }
                cursor += ch.len_utf8();
            }
            plain.push('\n');
        }

        let include_pattern = regex::Regex::new(r"(?:^|\s)@((?:[^\s\\]|\\ )+)")
            .expect("valid source include expression");
        let mut paths = Vec::new();
        let mut unique = HashSet::new();
        for found in include_pattern.captures_iter(&plain) {
            let raw = found.get(1).expect("include capture").as_str();
            let raw = raw
                .split('#')
                .next()
                .unwrap_or("")
                .trim_end_matches(|ch: char| ",.;:!?)]}".contains(ch))
                .replace("\\ ", " ");
            if raw.is_empty()
                || !raw.chars().next().is_some_and(|ch| {
                    ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | '/' | '~')
                })
            {
                continue;
            }
            let candidate = if let Some(relative) = raw.strip_prefix("~/") {
                match std::env::var_os("HOME") {
                    Some(home) => PathBuf::from(home).join(relative),
                    None => continue,
                }
            } else if raw.starts_with('/') {
                PathBuf::from(&raw)
            } else {
                file.parent().unwrap_or(Path::new(".")).join(&raw)
            };
            if let Some(extension) = candidate.extension().and_then(|ext| ext.to_str()) {
                if !matches!(
                    extension.to_ascii_lowercase().as_str(),
                    "md" | "txt" | "text" | "json" | "yaml" | "yml" | "toml"
                        | "xml" | "csv" | "html" | "htm" | "css" | "scss"
                        | "sass" | "less" | "js" | "ts" | "tsx" | "jsx"
                        | "mjs" | "cjs" | "py" | "sh" | "rs"
                ) {
                    continue;
                }
            }
            if unique.insert(candidate.clone()) {
                paths.push(candidate);
            }
        }
        paths
    }

    fn add_file(
        path: &Path,
        scope: &Path,
        is_rule: bool,
        seen: &mut HashSet<PathBuf>,
        output: &mut String,
    ) -> anyhow::Result<()> {
        add_file_inner(path, scope, is_rule, seen, output, 0)
    }

    fn add_file_inner(
        path: &Path,
        scope: &Path,
        is_rule: bool,
        seen: &mut HashSet<PathBuf>,
        output: &mut String,
        depth: usize,
    ) -> anyhow::Result<()> {
        if depth >= 5 {
            return Ok(());
        }
        let metadata = match fs::metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("Inspect project instruction: {}", path.display()))
            }
        };
        if !metadata.is_file() {
            return Ok(());
        }
        let canonical = path.canonicalize()?;
        if !canonical.starts_with(scope) {
            bail!(
                "Instruction file points outside its approved directory: {}",
                path.display()
            );
        }
        if seen.contains(&canonical) {
            return Ok(());
        }
        if seen.len() >= MAX_FILES {
            bail!("Too many project instruction files (limit {MAX_FILES})");
        }
        if metadata.len() > MAX_FILE_BYTES {
            bail!("Project instruction file too large: {}", path.display());
        }
        let bytes = fs::read(&canonical)
            .with_context(|| format!("Read project instructions: {}", path.display()))?;
        if bytes.len() as u64 > MAX_FILE_BYTES {
            bail!("Project instruction file too large: {}", path.display());
        }
        let content = String::from_utf8(bytes)
            .with_context(|| format!("Project instructions must be UTF-8: {}", path.display()))?;
        let body = if is_rule {
            rule_body(&content)
        } else {
            Some(content.as_str())
        };
        // Mark the parent before expansion to break circular references.
        // Included instructions are placed before the referring file so the
        // referring file retains the last-word precedence in the model prompt.
        seen.insert(canonical.clone());
        if let Some(body) = body.filter(|body| !body.trim().is_empty()) {
            for include in include_paths(body, &canonical) {
                let resolved = match include.canonicalize() {
                    Ok(resolved) => resolved,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(error) => {
                        return Err(error).with_context(|| {
                            format!("Resolve instruction include: {}", include.display())
                        })
                    }
                };
                if resolved.starts_with(scope) {
                    add_file_inner(&resolved, scope, false, seen, output, depth + 1)?;
                }
            }
            output.push_str(&format!(
                "\nProject instructions from {}:\n{}\n",
                path.display(),
                body.trim()
            ));
        }
        Ok(())
    }

    fn add_rules(
        directory: &Path,
        scope: &Path,
        depth: usize,
        visited: &mut HashSet<PathBuf>,
        seen: &mut HashSet<PathBuf>,
        output: &mut String,
    ) -> anyhow::Result<()> {
        if depth > MAX_RULE_DEPTH {
            anyhow::bail!("Project rules directory nesting exceeds {MAX_RULE_DEPTH}");
        }
        let metadata = match fs::metadata(directory) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        if !metadata.is_dir() {
            return Ok(());
        }
        let canonical = directory.canonicalize()?;
        if !canonical.starts_with(scope) {
            anyhow::bail!(
                "Project rules directory escapes its approved scope: {}",
                directory.display()
            );
        }
        if !visited.insert(canonical) {
            return Ok(());
        }
        let mut entries = fs::read_dir(directory)?
            .map(|entry| entry.map(|item| item.path()))
            .collect::<std::io::Result<Vec<_>>>()?;
        entries.sort();
        for entry in entries {
            let metadata = fs::metadata(&entry)?;
            if metadata.is_dir() {
                add_rules(&entry, scope, depth + 1, visited, seen, output)?;
            } else if metadata.is_file()
                && entry.extension().and_then(|extension| extension.to_str()) == Some("md")
            {
                add_file(&entry, scope, true, seen, output)?;
            }
        }
        Ok(())
    }

    if !enabled() {
        return Ok(String::new());
    }
    let root = root.canonicalize()?;
    let mut output = String::new();
    let mut seen = HashSet::new();
    let mut visited = HashSet::new();

    let config = crate::config::Config::path()?;
    if let Some(home) = config.parent() {
        match fs::metadata(home) {
            Ok(metadata) if metadata.is_dir() => {
                let home = home.canonicalize()?;
                add_file(
                    &home.join("AGENTS.md"),
                    &home,
                    false,
                    &mut seen,
                    &mut output,
                )?;
                add_rules(
                    &home.join("rules"),
                    &home,
                    0,
                    &mut visited,
                    &mut seen,
                    &mut output,
                )?;
            }
            Ok(_) => anyhow::bail!("Configuration home is not a directory: {}", home.display()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }

    let mut scopes = root
        .ancestors()
        .filter(|directory| directory.parent().is_some())
        .collect::<Vec<_>>();
    scopes.reverse();
    for directory in scopes {
        for relative in ["AGENTS.md", ".atlas/AGENTS.md", ".atlas/instructions.md"] {
            add_file(
                &directory.join(relative),
                directory,
                false,
                &mut seen,
                &mut output,
            )?;
        }
        add_rules(
            &directory.join(".atlas/rules"),
            directory,
            0,
            &mut visited,
            &mut seen,
            &mut output,
        )?;
        for relative in ["AGENTS.local.md", ".atlas/instructions.local.md"] {
            add_file(
                &directory.join(relative),
                directory,
                false,
                &mut seen,
                &mut output,
            )?;
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::Path, process::Command};

    fn run_git(root: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "git {:?} failed", args);
    }

    #[test]
    fn captures_real_git_once_and_keeps_snapshot_when_worktree_changes() {
        let temp = tempfile::tempdir().unwrap();
        run_git(temp.path(), &["init", "-q", "-b", "main"]);
        run_git(temp.path(), &["config", "user.name", "Test Author"]);
        run_git(
            temp.path(),
            &["config", "user.email", "test@example.invalid"],
        );
        fs::write(temp.path().join("main.txt"), "first").unwrap();
        run_git(temp.path(), &["add", "main.txt"]);
        run_git(temp.path(), &["commit", "-qm", "initial commit"]);
        fs::write(temp.path().join("main.txt"), "changed").unwrap();
        let snapshot = ConversationContext::capture(temp.path());
        fs::write(temp.path().join("main.txt"), "another change").unwrap();
        let mut message = String::new();
        snapshot.append_to(&mut message);
        assert!(message.contains("Current branch: main"));
        assert!(message.contains("Main branch (you will usually use this for PRs): main"));
        assert!(message.contains("M main.txt"));
        assert!(message.contains("initial commit"));
        assert!(message.contains("Today's date is "));
        assert_eq!(message, {
            let mut second = String::new();
            snapshot.append_to(&mut second);
            second
        });
        assert!(ConversationContext::capture(temp.path())
            .git_status
            .is_some());
    }

    #[test]
    fn non_repo_skips_git_but_keeps_date() {
        let temp = tempfile::tempdir().unwrap();
        let snapshot = ConversationContext::capture(temp.path());
        assert!(snapshot.git_status.is_none());
        let mut prompt = String::new();
        snapshot.append_to(&mut prompt);
        assert!(prompt.contains("Today's date is"));
        assert!(!prompt.contains("Current branch"));
    }

    #[test]
    fn truncate_status_on_char_boundary() {
        let temp = tempfile::tempdir().unwrap();
        run_git(temp.path(), &["init", "-q", "-b", "main"]);
        for i in 0..150 {
            fs::write(temp.path().join(format!("文件_{i:04}_很长的名字.txt")), "x").unwrap();
        }
        let snapshot = ConversationContext::capture(temp.path());
        let status = snapshot.git_status.unwrap();
        assert!(status.contains("truncated because it exceeds 2k characters"));
        assert!(status.contains("Current branch: main"));
    }
}
