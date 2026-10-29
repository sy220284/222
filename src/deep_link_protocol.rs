//! External URI trampoline for 2.1.88 src/utils/deepLink/protocolHandler.ts.
//! Uses the current binary instead of PATH lookup; launch data is direct argv.
use crate::{
    config::Config,
    deep_link,
    deep_link_banner,
    github_repo_mapping,
    terminal_launcher::{self, TerminalLaunch},
};
use anyhow::{Context, Result};
use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

pub fn prepare_launch(uri: &str, config: &Config) -> Result<TerminalLaunch> {
    let action = deep_link::parse_deep_link(uri)?;
    let cwd = action.cwd.as_deref().map(Path::new);
    let selected = github_repo_mapping::resolve_cwd(
        config, cwd, action.repo.as_deref()
    )?;
    let fetched = selected.resolved_repo.as_ref()
        .and_then(|_| deep_link_banner::read_last_fetch_time(&selected.cwd))
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX));
    Ok(TerminalLaunch {
        cwd: Some(selected.cwd),
        repo: selected.resolved_repo,
        last_fetch_ms: fetched,
        query: action.query,
    })
}

pub fn handle_deep_link_uri(uri: &str, config: &Config) -> Result<()> {
    let action = prepare_launch(uri, config)?;
    let binary = std::env::current_exe().context("Cannot resolve running executable")?;
    terminal_launcher::launch_in_terminal(&binary, &action)
}

pub fn known_macos_url_scheme_launch(bundle_id: &str, actual: Option<&str>) -> bool {
    cfg!(target_os = "macos")
        && actual.is_some_and(|actual| actual == bundle_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::PathBuf, process::Command};

    #[test]
    fn uri_prefill_is_not_executed_and_explicit_directory_wins() {
        let cfg = Config::default();
        let link = "atlas-cli://open?q=review+before+run&cwd=%2Ftmp%2Fproject&repo=team%2Frepo";
        let plan = prepare_launch(link, &cfg).unwrap();
        assert_eq!(plan.cwd, Some(PathBuf::from("/tmp/project")));
        assert!(plan.repo.is_none());
        assert_eq!(plan.query.as_deref(), Some("review before run"));
    }

    #[test]
    fn known_repository_uses_real_local_clone_or_home_fallback() {
        let repo = tempfile::tempdir().unwrap();
        assert!(Command::new("git").args(["init", "-q"])
            .current_dir(repo.path()).status().unwrap().success());
        fs::write(repo.path().join(".git/config"),
            "[remote \"origin\"]\nurl = git@github.com:team/repo.git\n").unwrap();
        let mut config = Config::default();
        assert!(github_repo_mapping::update_repo_path_mapping(
            &mut config, repo.path()).unwrap());
        let local = prepare_launch("atlas-cli://open?repo=team%2Frepo", &config).unwrap();
        assert_eq!(local.cwd, Some(repo.path().canonicalize().unwrap()));
        assert_eq!(local.repo.as_deref(), Some("team/repo"));
        let other = prepare_launch("atlas-cli://open?repo=team%2Fabsent", &config).unwrap();
        assert!(other.repo.is_none());
        assert_eq!(other.cwd, dirs::home_dir());
    }
}
