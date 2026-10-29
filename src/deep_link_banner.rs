//! External deep-link provenance banner and FETCH_HEAD freshness.
//! Original: src/utils/deepLink/banner.ts (2.1.88).
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

const STALE_FETCH_WARN: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const LONG_PREFILL_THRESHOLD: usize = 1000;

#[derive(Debug, Clone)]
pub struct DeepLinkBannerInfo {
    pub cwd: PathBuf,
    pub prefill_length: Option<usize>,
    pub repo: Option<String>,
    pub last_fetch: Option<SystemTime>,
}

fn compact_number(number: usize) -> String {
    if number < 1000 {
        return number.to_string();
    }
    let mut value = number as f64;
    let mut suffix = "";
    for unit in ["k", "m", "b", "t"] {
        value /= 1000.0;
        suffix = unit;
        if value < 999.5 {
            break;
        }
    }
    format!("{value:.1}{suffix}")
}

fn relative_time_ago(then: SystemTime, now: SystemTime) -> String {
    let (elapsed, future) = match now.duration_since(then) {
        Ok(elapsed) => (elapsed, false),
        Err(error) => (error.duration(), true),
    };
    let seconds = elapsed.as_secs();
    let (value, unit) = [
        (31_536_000, "y"),
        (2_592_000, "mo"),
        (604_800, "w"),
        (86_400, "d"),
        (3_600, "h"),
        (60, "m"),
        (1, "s"),
    ]
    .iter()
    .find(|(step, _)| seconds >= *step)
    .map(|(step, unit)| (seconds / step, *unit))
    .unwrap_or((0, "s"));
    if future {
        format!("in {value}{unit}")
    } else {
        format!("{value}{unit} ago")
    }
}

fn home_shortcut(path: &Path) -> String {
    let home = dirs::home_dir();
    if let Some(home) = home {
        if path == home {
            return "~".into();
        }
        if let Ok(relative) = path.strip_prefix(&home) {
            return format!("~/{}", relative.display());
        }
    }
    path.display().to_string()
}

pub fn build_deep_link_banner(info: &DeepLinkBannerInfo) -> String {
    build_deep_link_banner_at(info, SystemTime::now())
}

pub fn build_deep_link_banner_at(info: &DeepLinkBannerInfo, now: SystemTime) -> String {
    let mut lines = vec![format!(
        "This session was opened by an external deep link in {}",
        home_shortcut(&info.cwd)
    )];
    if let Some(repo) = info.repo.as_ref().filter(|repo| !repo.is_empty()) {
        let age = info
            .last_fetch
            .map(|time| relative_time_ago(time, now))
            .unwrap_or_else(|| "never".into());
        let stale = info.last_fetch.is_none_or(|then| {
            now.duration_since(then)
                .is_ok_and(|age| age > STALE_FETCH_WARN)
        });
        lines.push(format!(
            "Resolved {repo} from local clones · last fetched {age}{}",
            if stale {
                " — AGENTS.md may be stale"
            } else {
                ""
            }
        ));
    }
    if let Some(length) = info.prefill_length.filter(|length| *length != 0) {
        lines.push(if length > LONG_PREFILL_THRESHOLD {
            format!(
                "The prompt below ({} chars) was supplied by the link — scroll to review the entire prompt before pressing Enter.",
                compact_number(length)
            )
        } else {
            "The prompt below was supplied by the link — review carefully before pressing Enter."
                .into()
        });
    }
    lines.join("\n")
}

fn mtime_of(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).ok()?.modified().ok()
}

/// Return the newest FETCH_HEAD mtime from the current worktree or its
/// shared git directory. Never fetches a remote or changes repository state.
pub fn read_last_fetch_time(cwd: &Path) -> Option<SystemTime> {
    let git_dir = crate::git_filesystem::resolve_git_dir(cwd)?;
    let own = mtime_of(&git_dir.join("FETCH_HEAD"));
    let shared = crate::git_filesystem::get_common_dir(&git_dir)
        .and_then(|common| mtime_of(&common.join("FETCH_HEAD")));
    match (own, shared) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (local, common) => local.or(common),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_origin_repo_freshness_and_long_prefill_warning() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000);
        let mut info = DeepLinkBannerInfo {
            cwd: PathBuf::from("/workspace/repo"),
            prefill_length: Some(1001),
            repo: Some("team/repo".into()),
            last_fetch: Some(now - Duration::from_secs(8 * 86_400)),
        };
        let banner = build_deep_link_banner_at(&info, now);
        assert!(banner.contains("external deep link in /workspace/repo"));
        assert!(banner.contains("Resolved team/repo"));
        assert!(banner.contains("1w ago"));
        assert!(banner.contains("AGENTS.md may be stale"));
        assert!(banner.contains("1.0k chars"));
        assert!(banner.contains("scroll to review the entire prompt"));
        info.prefill_length = Some(1000);
        info.last_fetch = Some(now - Duration::from_secs(60));
        let shorter = build_deep_link_banner_at(&info, now);
        assert!(!shorter.contains("may be stale"));
        assert!(!shorter.contains("scroll to review"));
        info.prefill_length = None;
        info.last_fetch = None;
        let missing = build_deep_link_banner_at(&info, now);
        assert!(missing.contains("last fetched never"));
        assert!(!missing.contains("prompt below"));
    }

    #[test]
    fn git_fetch_time_uses_current_worktree_and_shared_gitdir() {
        let root = tempfile::tempdir().unwrap();
        let repo = root.path();
        let git = repo.join(".git");
        fs::create_dir_all(git.join("worktrees/a")).unwrap();
        let worktree = repo.join("worktree-a");
        fs::create_dir(&worktree).unwrap();
        let private = git.join("worktrees/a");
        fs::write(private.join("commondir"), "../..\n").unwrap();
        fs::write(
            worktree.join(".git"),
            format!("gitdir: {}\n", private.display()),
        )
        .unwrap();
        assert_eq!(read_last_fetch_time(&worktree), None);
        fs::write(git.join("FETCH_HEAD"), "fetched").unwrap();
        let common = read_last_fetch_time(&worktree).unwrap();
        fs::write(private.join("FETCH_HEAD"), "more recent").unwrap();
        assert!(read_last_fetch_time(&worktree).unwrap() >= common);
        crate::git_filesystem::clear_resolve_git_dir_cache();
    }
}
