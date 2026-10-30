//! Repository clone MRU mapping for deep links.
//! Port of 2.1.88 src/utils/githubRepoPathMapping.ts.
use crate::{config::Config, detect_repository::parse_github_repository, git_filesystem};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use unicode_normalization::UnicodeNormalization;

fn key(repo: &str) -> String {
    repo.to_lowercase()
}

fn git_root(path: &Path) -> Option<&Path> {
    path.ancestors().find(|directory| {
        let git = directory.join(".git");
        git.is_dir() || git.is_file()
    })
}

/// Promote an actual checkout root, never the active subdirectory.
pub fn update_repo_path_mapping(config: &mut Config, cwd: &Path) -> Result<bool> {
    let Some(repo) =
        git_filesystem::get_remote_url_for_dir(cwd).and_then(|raw| parse_github_repository(&raw))
    else {
        return Ok(false);
    };
    let root = git_root(cwd).unwrap_or(cwd);
    // githubRepoPathMapping.ts falls back to the original path when
    // realpath fails (for example, on a restricted mounted checkout).
    // A path lookup failure must not prevent tracking a known repository.
    let normalized = root
        .canonicalize()
        .unwrap_or_else(|_| root.to_path_buf())
        .to_string_lossy()
        .nfc()
        .collect::<String>();
    let existing = config.github_repo_paths.entry(key(&repo)).or_default();
    if existing.first().is_some_and(|path| path == &normalized) {
        return Ok(false);
    }
    existing.retain(|value| value != &normalized);
    existing.insert(0, normalized);
    Ok(true)
}

pub fn get_known_paths_for_repo(config: &Config, repo: &str) -> Vec<PathBuf> {
    config
        .github_repo_paths
        .get(&key(repo))
        .into_iter()
        .flatten()
        .map(PathBuf::from)
        .collect()
}

pub fn filter_existing_paths(paths: &[PathBuf]) -> Vec<PathBuf> {
    paths.iter().filter(|path| path.exists()).cloned().collect()
}

pub fn validate_repo_at_path(path: &Path, expected_repo: &str) -> bool {
    git_filesystem::get_remote_url_for_dir(path)
        .and_then(|raw| parse_github_repository(&raw))
        .is_some_and(|actual| actual.eq_ignore_ascii_case(expected_repo))
}

pub fn remove_path_from_repo(config: &mut Config, repo: &str, path: &Path) -> bool {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let normalized = canonical.to_string_lossy().nfc().collect::<String>();
    let entry = key(repo);
    let Some(paths) = config.github_repo_paths.get_mut(&entry) else {
        return false;
    };
    let previous = paths.len();
    paths.retain(|value| value != &normalized);
    let changed = previous != paths.len();
    if paths.is_empty() {
        config.github_repo_paths.remove(&entry);
    }
    changed
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedDeepLinkCwd {
    pub cwd: PathBuf,
    pub resolved_repo: Option<String>,
}

/// Explicit cwd wins; otherwise select the most recently used valid clone.
/// Unknown repositories fall back to home, just as the original handler.
pub fn resolve_cwd(
    config: &Config,
    cwd: Option<&Path>,
    repo: Option<&str>,
) -> Result<ResolvedDeepLinkCwd> {
    if let Some(path) = cwd {
        return Ok(ResolvedDeepLinkCwd {
            cwd: path.to_path_buf(),
            resolved_repo: None,
        });
    }
    if let Some(repo) = repo {
        // protocolHandler.ts calls getKnownPathsForRepo + filterExistingPaths:
        // a previously recorded, existing clone wins even when its remote
        // was later renamed or removed. validateRepoAtPath is a separate
        // exported utility, not a prerequisite for resolving deep links.
        if let Some(path) = filter_existing_paths(&get_known_paths_for_repo(config, repo))
            .into_iter()
            .next()
        {
            return Ok(ResolvedDeepLinkCwd {
                cwd: path,
                resolved_repo: Some(repo.to_owned()),
            });
        }
    }
    Ok(ResolvedDeepLinkCwd {
        cwd: dirs::home_dir().context("Cannot find a home directory for deep links")?,
        resolved_repo: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, process::Command};

    #[test]
    fn mru_mapping_tracks_real_checkout_and_recovers_removed_clone() {
        let mut config = Config::default();
        let root = tempfile::tempdir().unwrap();
        let project = root.path();
        assert!(Command::new("git")
            .args(["init", "-q"])
            .current_dir(project)
            .status()
            .unwrap()
            .success());
        assert!(Command::new("git")
            .args(["remote", "add", "origin", "git@github.com:Acme/Tools.git"])
            .current_dir(project)
            .status()
            .unwrap()
            .success());
        fs::create_dir(project.join("src")).unwrap();
        assert!(update_repo_path_mapping(&mut config, &project.join("src")).unwrap());
        assert!(!update_repo_path_mapping(&mut config, project).unwrap());
        let known = get_known_paths_for_repo(&config, "acme/TOOLS");
        assert_eq!(known.len(), 1);
        assert_eq!(known[0], project.canonicalize().unwrap());
        let found = resolve_cwd(&config, None, Some("Acme/Tools")).unwrap();
        assert_eq!(found.cwd, known[0]);
        assert_eq!(found.resolved_repo.as_deref(), Some("Acme/Tools"));
        assert!(!validate_repo_at_path(project, "another/repo"));
        // The source deep-link handler does not revalidate a tracked
        // checkout's remote. A user may still open it after remote removal.
        assert!(Command::new("git")
            .args(["remote", "remove", "origin"])
            .current_dir(project)
            .status()
            .unwrap()
            .success());
        assert!(!validate_repo_at_path(project, "Acme/Tools"));
        let reopened = resolve_cwd(&config, None, Some("Acme/Tools")).unwrap();
        assert_eq!(reopened.cwd, project.canonicalize().unwrap());
        assert_eq!(reopened.resolved_repo.as_deref(), Some("Acme/Tools"));
        assert!(remove_path_from_repo(&mut config, "Acme/Tools", project));
        assert!(get_known_paths_for_repo(&config, "acme/tools").is_empty());
        let missing = resolve_cwd(&config, None, Some("Acme/Tools")).unwrap();
        assert!(missing.resolved_repo.is_none());
        assert_eq!(
            resolve_cwd(&config, Some(project), Some("wrong/repo"))
                .unwrap()
                .cwd,
            project
        );
    }

    #[test]
    fn tracked_paths_promote_to_front_and_preserve_other_clones() {
        let mut cfg = Config::default();
        cfg.github_repo_paths.insert(
            "team/repo".into(),
            vec!["/tmp/first".into(), "/tmp/second".into()],
        );
        assert!(remove_path_from_repo(
            &mut cfg,
            "TEAM/REPO",
            Path::new("/tmp/first")
        ));
        assert_eq!(
            get_known_paths_for_repo(&cfg, "TEAM/REPO"),
            vec![PathBuf::from("/tmp/second")]
        );
        assert!(!remove_path_from_repo(
            &mut cfg,
            "team/repo",
            Path::new("/tmp/missing")
        ));
    }
}
