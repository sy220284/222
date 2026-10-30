//! Source-driven Rust implementations of built-in review/commit/PR prompts.
//! Prompt preparation uses fixed argv Git commands; no user text is executed
//! as a shell command. The Agent still applies normal tool permissions.
use anyhow::{bail, Context, Result};
use std::{path::Path, process::Command};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Review,
    Commit,
    CommitPushPr,
}

fn git(root: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .with_context(|| format!("Cannot start Git in {}", root.display()))?;
    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    String::from_utf8(output.stdout).context("Git output is not valid UTF-8")
}

fn git_context(root: &Path) -> Result<(String, String, String, String)> {
    git(root, &["rev-parse", "--show-toplevel"])?;
    Ok((
        git(root, &["status", "--short"])?,
        git(root, &["diff", "HEAD"])?,
        git(root, &["branch", "--show-current"])?.trim().to_owned(),
        git(root, &["log", "--oneline", "-10"])?,
    ))
}

fn default_branch(root: &Path) -> Result<String> {
    if let Ok(value) = git(
        root,
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
    ) {
        if let Some(branch) = value.trim().strip_prefix("origin/") {
            return Ok(branch.to_owned());
        }
    }
    for name in ["main", "master"] {
        if git(
            root,
            &[
                "show-ref",
                "--verify",
                "--quiet",
                &format!("refs/heads/{name}"),
            ],
        )
        .is_ok()
        {
            return Ok(name.to_owned());
        }
    }
    bail!("No configured default Git branch (origin/HEAD, main, master)")
}

fn user_instructions(args: &str) -> String {
    let args = args.trim();
    if args.is_empty() {
        String::new()
    } else {
        format!("\n\n## 用户补充要求\n\n{args}")
    }
}

pub fn prompt(action: Action, root: &Path, args: &str) -> Result<String> {
    match action {
        Action::Review => {
            let identifier = args.trim();
            if !identifier.is_empty()
                && (!identifier.chars().all(|c| c.is_ascii_digit()) || identifier.starts_with('0'))
            {
                bail!("PR number must be a positive decimal integer");
            }
            Ok(format!(
                r#"You are an expert code reviewer working in the user's current Git repository.
1. If no pull request number is supplied, run `gh pr list` to find open PRs.
2. If a PR number is supplied, run `gh pr view NUMBER` for the details.
3. Run `gh pr diff NUMBER` to see the entire proposed change.
4. Examine the full diff for correctness, code quality, project conventions, performance, tests and security.
5. Report specific problems with file and line references and actionable fixes; give an overall summary.
Never change files or PR state as part of a review.

PR number: {identifier}"#
            ))
        }
        Action::Commit => {
            let (status, diff, branch, log) = git_context(root)?;
            Ok(format!(
                r#"Create one Git commit from the user's current changes.

## Actual local Git context
Git status:
```text
{status}
```
Git diff against HEAD:
```diff
{diff}
```
Branch: {branch}
Recent commits:
```text
{log}
```

## Git safety rules
- Inspect both staged and unstaged changes; do not create an empty commit.
- Never update Git config, amend commits, force push, or skip Git hooks unless specifically requested.
- Never stage secrets such as .env files, API keys or credentials.
- Make one NEW commit with a concise, accurate Chinese commit message.
- Respect the current tool permission mode; do not grant yourself additional permissions.
- Return the actual commit SHA and changed files after execution.
{extra}"#,
                extra = user_instructions(args)
            ))
        }
        Action::CommitPushPr => {
            let (status, diff, branch, log) = git_context(root)?;
            let base = default_branch(root)?;
            let relative_diff = git(root, &["diff", &format!("{base}...HEAD")])?;
            Ok(format!(
                r#"Create a reviewed Git commit, push a branch and create or update a pull request.

## Actual local Git context
Current branch: {branch}
Default branch: {base}
Git status:
```text
{status}
```
Changes since HEAD:
```diff
{diff}
```
All changes against base:
```diff
{relative_diff}
```
Recent commits:
```text
{log}
```

## Git safety and workflow
1. Inspect all PR changes, including earlier branch commits. If on {base}, create a new feature branch first.
2. Exclude sensitive files; never alter git config, skip hooks or overwrite/force-push history.
3. Create a new commit where there are changes to commit (do not amend or make empty commits).
4. Push the feature branch to origin. Confirm push succeeded before changing a PR.
5. Check if a PR already exists, then update it; otherwise create a new PR. **PR title and description must be in Chinese**. Include a summary and testing details.
6. Use standard explicit tool approvals; do not silently elevate Git or GitHub permissions.
7. Return the actual PR URL. Do not merge the PR unless the user explicitly asks to merge.
{extra}"#,
                extra = user_instructions(args)
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn repo() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        fn run(root: &Path, args: &[&str]) {
            let status = Command::new("git")
                .arg("-C")
                .arg(root)
                .args(args)
                .status()
                .unwrap();
            assert!(status.success());
        }
        run(root.path(), &["init", "-q", "-b", "main"]);
        run(root.path(), &["config", "user.name", "Test"]);
        run(
            root.path(),
            &["config", "user.email", "test@example.invalid"],
        );
        std::fs::write(root.path().join("sample.txt"), "before\n").unwrap();
        run(root.path(), &["add", "sample.txt"]);
        run(root.path(), &["commit", "-qm", "Initialize"]);
        root
    }
    #[test]
    fn review_prompt_follows_pr_number_and_never_executes_user_input() {
        let root = tempfile::tempdir().unwrap();
        let review = prompt(Action::Review, root.path(), "123").unwrap();
        assert!(review.contains("PR number: 123"));
        assert!(review.contains("gh pr diff"));
        assert!(review.contains("Never change files"));
        assert!(prompt(Action::Review, root.path(), "123; rm -rf /").is_err());
        assert!(prompt(Action::Review, root.path(), "-1").is_err());
    }
    #[test]
    fn commit_prompt_includes_real_changed_worktree_and_does_not_commit() {
        let root = repo();
        std::fs::write(root.path().join("sample.txt"), "after\n").unwrap();
        let result = prompt(Action::Commit, root.path(), "").unwrap();
        assert!(result.contains("-before"));
        assert!(result.contains("+after"));
        assert!(result.contains("sample.txt"));
        assert!(result.contains("Chinese commit message"));
        assert_eq!(
            git(root.path(), &["status", "--porcelain"]).unwrap().trim(),
            "M sample.txt"
        );
    }
    #[test]
    fn pr_prompt_resolves_actual_default_branch_and_requires_chinese_pr() {
        let root = repo();
        let result = prompt(Action::CommitPushPr, root.path(), "检查边界").unwrap();
        assert!(result.contains("Default branch: main"));
        assert!(result.contains("PR title and description must be in Chinese"));
        assert!(result.contains("检查边界"));
        assert!(prompt(
            Action::CommitPushPr,
            tempfile::tempdir().unwrap().path(),
            ""
        )
        .is_err());
    }
}
