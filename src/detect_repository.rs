//! Provider-neutral repository detection from 2.1.88 src/utils/detectRepository.ts.
#![allow(dead_code)]
use regex::Regex;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Command,
    sync::{Mutex, OnceLock},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRepository {
    pub host: String,
    pub owner: String,
    pub name: String,
}

fn repository_cache() -> &'static Mutex<HashMap<PathBuf, Option<ParsedRepository>>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Option<ParsedRepository>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn clear_repository_caches() {
    repository_cache()
        .lock()
        .expect("repository cache poisoned")
        .clear();
}

fn looks_like_real_hostname(host: &str) -> bool {
    let Some((_, top_level)) = host.rsplit_once('.') else {
        return false;
    };
    !top_level.is_empty() && top_level.bytes().all(|b| b.is_ascii_alphabetic())
}

pub fn parse_git_remote(input: &str) -> Option<ParsedRepository> {
    let remote = input.trim();
    static SCP: OnceLock<Regex> = OnceLock::new();
    let scp = SCP.get_or_init(|| {
        Regex::new(r"^git@([^:]+):([^/]+)/([^/]+?)(?:\.git)?$").expect("valid git remote regex")
    });
    if let Some(caps) = scp.captures(remote) {
        let host = caps.get(1)?.as_str();
        if !looks_like_real_hostname(host) {
            return None;
        }
        return Some(ParsedRepository {
            host: host.to_owned(),
            owner: caps.get(2)?.as_str().to_owned(),
            name: caps.get(3)?.as_str().to_owned(),
        });
    }

    static URL: OnceLock<Regex> = OnceLock::new();
    let url = URL.get_or_init(|| {
        Regex::new(r"^(https?|ssh|git)://(?:[^@]+@)?([^/:]+(?::\d+)?)/([^/]+)/([^/]+?)(?:\.git)?$")
            .expect("valid git URL regex")
    });
    let captures = url.captures(remote)?;
    let scheme = captures.get(1)?.as_str();
    let host_with_port = captures.get(2)?.as_str();
    let host = host_with_port.split(':').next()?;
    if !looks_like_real_hostname(host) {
        return None;
    }
    Some(ParsedRepository {
        host: if matches!(scheme, "http" | "https") {
            host_with_port.to_owned()
        } else {
            host.to_owned()
        },
        owner: captures.get(3)?.as_str().to_owned(),
        name: captures.get(4)?.as_str().to_owned(),
    })
}

pub fn parse_github_repository(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if let Some(repo) = parse_git_remote(trimmed) {
        return (repo.host == "github.com").then(|| format!("{}/{}", repo.owner, repo.name));
    }
    if trimmed.contains("://") || trimmed.contains('@') {
        return None;
    }
    let (owner, repo) = trimmed.split_once('/')?;
    if owner.is_empty() || repo.is_empty() || repo.contains('/') {
        return None;
    }
    let repo = repo.strip_suffix(".git").unwrap_or(repo);
    Some(format!("{owner}/{repo}"))
}

/// One safe git subprocess is used to resolve the existing origin; it never
/// contacts a remote server. The parsed host/repo is safe to display even if
/// the configured origin URL contains user information.
pub fn detect_current_repository_with_host(root: &Path) -> Option<ParsedRepository> {
    let root = root.canonicalize().ok()?;
    if let Some(value) = repository_cache()
        .lock()
        .expect("repository cache poisoned")
        .get(&root)
    {
        return value.clone();
    }
    let from_command = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|remote| parse_git_remote(&remote));
    // The original Git filesystem utility reads origin from the config when
    // spawning Git is unavailable. Local reads do not access the network.
    let value = from_command.or_else(|| {
        let git_directory = root.join(".git");
        crate::git_config_parser::parse_git_config_value(
            &git_directory, "remote", Some("origin"), "url"
        ).and_then(|remote| parse_git_remote(&remote))
    });
    repository_cache()
        .lock()
        .expect("repository cache poisoned")
        .insert(root, value.clone());
    value
}

pub fn detect_current_repository(root: &Path) -> Option<String> {
    let repo = detect_current_repository_with_host(root)?;
    (repo.host == "github.com").then(|| format!("{}/{}", repo.owner, repo.name))
}

pub fn get_cached_repository(root: &Path) -> Option<String> {
    let root = root.canonicalize().ok()?;
    let cache = repository_cache()
        .lock()
        .expect("repository cache poisoned");
    let repo = cache.get(&root)?.as_ref()?;
    (repo.host == "github.com").then(|| format!("{}/{}", repo.owner, repo.name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn source_remote_variants_enterprise_ports_aliases_and_dotted_names() {
        assert_eq!(
            parse_git_remote(" git@github.com:owner/repo.git "),
            Some(ParsedRepository {
                host: "github.com".into(),
                owner: "owner".into(),
                name: "repo".into(),
            })
        );
        assert_eq!(
            parse_git_remote("https://ghe.example.com:8443/owner/cc.kurs.web.git")
                .unwrap()
                .host,
            "ghe.example.com:8443"
        );
        assert_eq!(
            parse_git_remote("ssh://git@ghe.example.com:2222/team/repo.git")
                .unwrap()
                .host,
            "ghe.example.com"
        );
        assert_eq!(
            parse_git_remote("git://github.com/user/nested.git")
                .unwrap()
                .name,
            "nested"
        );
        assert!(parse_git_remote("git@github.com-work:owner/repo.git").is_none());
        assert!(parse_git_remote("ssh://git@personal-alias/x/y").is_none());
        assert!(parse_git_remote("https://github.com/user/name/extra").is_none());
        assert_eq!(
            parse_github_repository("https://github.com/user/repo.git"),
            Some("user/repo".into())
        );
        assert_eq!(
            parse_github_repository("owner/cc.kurs.web.git"),
            Some("owner/cc.kurs.web".into())
        );
        assert!(parse_github_repository("https://ghe.example.com/team/repo").is_none());
    }

    #[test]
    fn existing_git_config_value_can_detect_remote_without_network() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        std::fs::write(
            dir.path().join(".git/config"),
            "[remote \"origin\"]\n URL = git@github.com:team/repo.git\n"
        ).unwrap();
        assert_eq!(
            crate::git_config_parser::parse_git_config_value(
                &dir.path().join(".git"), "remote", Some("origin"), "url"
            ).and_then(|raw| parse_git_remote(&raw))
                .map(|remote| format!("{}/{}", remote.owner, remote.name)),
            Some("team/repo".into())
        );
    }

    #[test]
    fn real_git_origin_cache_is_scoped_to_workspace_and_can_be_cleared() {
        clear_repository_caches();
        let root = tempfile::tempdir().unwrap();
        assert!(Command::new("git")
            .arg("-C")
            .arg(root.path())
            .args(["init", "-q"])
            .status()
            .unwrap()
            .success());
        assert!(Command::new("git")
            .arg("-C")
            .arg(root.path())
            .args(["remote", "add", "origin", "git@github.com:team/first.git"])
            .status()
            .unwrap()
            .success());
        assert_eq!(
            detect_current_repository(root.path()),
            Some("team/first".into())
        );
        assert_eq!(
            get_cached_repository(root.path()),
            Some("team/first".into())
        );
        assert!(Command::new("git")
            .arg("-C")
            .arg(root.path())
            .args([
                "remote",
                "set-url",
                "origin",
                "https://github.com/team/second.git"
            ])
            .status()
            .unwrap()
            .success());
        assert_eq!(
            detect_current_repository(root.path()),
            Some("team/first".into())
        );
        clear_repository_caches();
        assert_eq!(
            detect_current_repository(root.path()),
            Some("team/second".into())
        );
    }
}
