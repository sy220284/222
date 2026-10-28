use anyhow::{bail, Context, Result};
use std::{path::{Path, PathBuf}, process::Stdio, time::Duration};
use tokio::{process::Command, time::timeout};
use uuid::Uuid;

pub struct Worktree {
    repo: PathBuf,
    pub path: PathBuf,
    pub branch: String,
}

async fn git(cwd: &Path, args: &[&str]) -> Result<String> {
    let output = timeout(Duration::from_secs(60), Command::new("git")
        .arg("-C").arg(cwd).args(args)
        .stdin(Stdio::null()).kill_on_drop(true).output())
        .await.context("Git operation timed out")??;
    if !output.status.success() {
        bail!("Git {} failed: {}", args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

impl Worktree {
    /// Create an independent checkout from HEAD. It never modifies the source
    /// working tree or implicitly merges changes back into it.
    pub async fn create(workspace: &Path) -> Result<Self> {
        debug_assert!(crate::compat_helpers::is_worktree_mode_enabled());
        let original = git(workspace, &["rev-parse", "--show-toplevel"]).await?;
        let repo = PathBuf::from(original).canonicalize()?;
        let id = Uuid::new_v4().simple().to_string();
        let branch = format!("atlas-agent/{id}");
        let base = std::env::temp_dir().join("atlas-agent-worktrees");
        tokio::fs::create_dir_all(&base).await?;
        let path = base.join(&id);
        let rendered = path.to_string_lossy().to_string();
        git(&repo, &["worktree", "add", "-b", &branch, &rendered, "HEAD"]).await?;
        Ok(Self { repo, path, branch })
    }

    /// Keep a changed worktree for review; remove a clean worktree and branch.
    pub async fn finish(self) -> Result<String> {
        let changes = git(&self.path, &["status", "--porcelain"]).await?;
        if !changes.is_empty() {
            return Ok(format!(
                "Changes retained for review at {} (branch {}). No merge was performed.",
                self.path.display(), self.branch
            ));
        }
        let rendered = self.path.to_string_lossy().to_string();
        git(&self.repo, &["worktree", "remove", &rendered]).await?;
        git(&self.repo, &["branch", "-D", &self.branch]).await?;
        Ok("Clean isolated worktree removed after completion.".into())
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn keeps_modified_isolation_and_does_not_modify_parent() {
        let root = tempfile::tempdir().unwrap();
        let repo = root.path();
        git(repo, &["init", "-q"]).await.unwrap();
        tokio::fs::write(repo.join("original.txt"), "original").await.unwrap();
        git(repo, &["add", "original.txt"]).await.unwrap();
        git(repo, &["-c","user.name=Test","-c","user.email=test@example.com",
            "commit","-qm","initial"]).await.unwrap();
        let tree = Worktree::create(repo).await.unwrap();
        assert!(tree.path.join("original.txt").exists());
        tokio::fs::write(tree.path.join("new.txt"), "isolated").await.unwrap();
        let p = tree.path.clone();
        let summary = tree.finish().await.unwrap();
        assert!(summary.contains("retained"));
        assert!(!repo.join("new.txt").exists());
        git(repo, &["worktree", "remove", "--force", &p.to_string_lossy()])
            .await.unwrap();
    }

    #[tokio::test]
    async fn cleans_unchanged_worktree() {
        let root = tempfile::tempdir().unwrap();
        let repo = root.path();
        git(repo, &["init", "-q"]).await.unwrap();
        tokio::fs::write(repo.join("original.txt"), "original").await.unwrap();
        git(repo, &["add", "original.txt"]).await.unwrap();
        git(repo, &["-c","user.name=Test","-c","user.email=test@example.com",
            "commit","-qm","initial"]).await.unwrap();
        let tree = Worktree::create(repo).await.unwrap();
        let path = tree.path.clone();
        let result = tree.finish().await.unwrap();
        assert!(result.contains("removed"));
        assert!(!path.exists());
    }
}
