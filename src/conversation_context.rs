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
    let shortened = if status.chars().count() > MAX_STATUS_CHARS {
        format!(
            "{}\n... (truncated because it exceeds 2k characters. Run git status for more information)",
            status.chars().take(MAX_STATUS_CHARS).collect::<String>()
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
