//! Read-only CLI diff view based on commands/diff/diff.tsx and DiffDialog.
//! Uses fixed Git argv, bypassing pager, external diff and textconv hooks.
use anyhow::{bail, Context, Result};
use std::{path::Path, process::Command};

fn git(root: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .env("GIT_PAGER", "cat")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args(args)
        .output()
        .context("Cannot start Git for diff view")?;
    if !output.status.success() {
        bail!(
            "Git diff failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    String::from_utf8(output.stdout).context("Git diff contains invalid UTF-8")
}

pub fn render(root: &Path) -> Result<String> {
    git(root, &["rev-parse", "--show-toplevel"])?;
    let status = git(root, &["status", "--short", "--untracked-files=normal"])?;
    let staged = git(
        root,
        &[
            "diff",
            "--cached",
            "--no-ext-diff",
            "--no-textconv",
            "--no-color",
        ],
    )?;
    let unstaged = git(
        root,
        &["diff", "--no-ext-diff", "--no-textconv", "--no-color"],
    )?;
    let mut output = String::from("## Workspace changes\n\n");
    if status.trim().is_empty() {
        return Ok("Working tree clean; no staged or unstaged changes.".into());
    }
    output.push_str("### Git status\n");
    output.push_str(&status);
    if !staged.trim().is_empty() {
        output.push_str("\n### Staged changes\n");
        output.push_str(&staged);
    }
    if !unstaged.trim().is_empty() {
        output.push_str("\n### Unstaged changes\n");
        output.push_str(&unstaged);
    }
    if staged.trim().is_empty() && unstaged.trim().is_empty() {
        output.push_str("\nOnly untracked or otherwise un-diffable files are present.\n");
    }
    Ok(output)
}
