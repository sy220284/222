//! Native Git filesystem state and per-workspace cache.
//! Mirrors refs, worktrees, packed refs, shallow state and remote lookup
//! without network access or Git subprocesses.
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

fn directory_cache() -> &'static Mutex<HashMap<PathBuf, Option<PathBuf>>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Option<PathBuf>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn clear_resolve_git_dir_cache() {
    directory_cache().lock().unwrap().clear();
    reset_git_file_watcher();
}

pub fn resolve_git_dir(start: &Path) -> Option<PathBuf> {
    let start = start.canonicalize().ok()?;
    if let Some(cached) = directory_cache().lock().unwrap().get(&start) {
        return cached.clone();
    }
    let result = start.ancestors().find_map(|directory| {
        let git = directory.join(".git");
        if git.is_dir() {
            return Some(git);
        }
        let contents = fs::read_to_string(&git).ok()?;
        let raw = contents.trim().strip_prefix("gitdir:")?.trim();
        let target = Path::new(raw);
        let target = if target.is_absolute() {
            target.to_path_buf()
        } else {
            directory.join(target)
        };
        target.canonicalize().ok()
    });
    directory_cache().lock().unwrap().insert(start, result.clone());
    result
}

pub fn get_common_dir(git_dir: &Path) -> Option<PathBuf> {
    let raw = fs::read_to_string(git_dir.join("commondir")).ok()?;
    let target = Path::new(raw.trim());
    let target = if target.is_absolute() {
        target.to_path_buf()
    } else {
        git_dir.join(target)
    };
    target.canonicalize().ok()
}

pub fn is_safe_ref_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && !name.starts_with('/')
        && !name.contains("..")
        && name.split('/').all(|part| !part.is_empty() && part != ".")
        && name.bytes().all(|byte| byte.is_ascii_alphanumeric()
            || matches!(byte, b'/' | b'.' | b'_' | b'+' | b'-' | b'@'))
}

pub fn is_valid_git_sha(hash: &str) -> bool {
    matches!(hash.len(), 40 | 64)
        && hash.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitHead {
    Branch(String),
    Detached(String),
}

pub fn read_git_head(git_dir: &Path) -> Option<GitHead> {
    let contents = fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let value = contents.trim();
    if let Some(reference) = value.strip_prefix("ref:") {
        let reference = reference.trim();
        if let Some(branch) = reference.strip_prefix("refs/heads/") {
            return is_safe_ref_name(branch).then(|| GitHead::Branch(branch.to_owned()));
        }
        if !is_safe_ref_name(reference) {
            return None;
        }
        return Some(GitHead::Detached(resolve_ref(git_dir, reference).unwrap_or_default()));
    }
    is_valid_git_sha(value).then(|| GitHead::Detached(value.to_owned()))
}

pub fn resolve_ref(git_dir: &Path, reference: &str) -> Option<String> {
    let mut visited = HashSet::new();
    resolve_ref_inner(git_dir, reference, &mut visited)
}

fn resolve_ref_inner(
    git_dir: &Path,
    reference: &str,
    visited: &mut HashSet<(PathBuf, String)>,
) -> Option<String> {
    if !is_safe_ref_name(reference)
        || !visited.insert((git_dir.to_path_buf(), reference.to_owned()))
    {
        return None;
    }
    if let Some(value) = resolve_ref_in_dir(git_dir, reference, visited) {
        return Some(value);
    }
    let common = get_common_dir(git_dir)?;
    if common == git_dir {
        return None;
    }
    resolve_ref_in_dir(&common, reference, visited)
}

fn resolve_ref_in_dir(
    directory: &Path,
    reference: &str,
    visited: &mut HashSet<(PathBuf, String)>,
) -> Option<String> {
    if let Ok(raw) = fs::read_to_string(directory.join(reference)) {
        let value = raw.trim();
        if let Some(symref) = value.strip_prefix("ref:") {
            let symref = symref.trim();
            return is_safe_ref_name(symref)
                .then(|| resolve_ref_inner(directory, symref, visited))
                .flatten();
        }
        return is_valid_git_sha(value).then(|| value.to_owned());
    }
    let packed = fs::read_to_string(directory.join("packed-refs")).ok()?;
    for line in packed.split('\n') {
        if line.starts_with('#') || line.starts_with('^') {
            continue;
        }
        if let Some((sha, found)) = line.split_once(' ') {
            if found == reference {
                return is_valid_git_sha(sha).then(|| sha.to_owned());
            }
        }
    }
    None
}

pub fn read_raw_symref(git_dir: &Path, ref_path: &str, branch_prefix: &str) -> Option<String> {
    if !is_safe_ref_name(ref_path) {
        return None;
    }
    let raw = fs::read_to_string(git_dir.join(ref_path)).ok()?;
    let target = raw.trim().strip_prefix("ref:")?.trim();
    let branch = target.strip_prefix(branch_prefix)?;
    is_safe_ref_name(branch).then(|| branch.to_owned())
}

pub fn get_head_for_dir(cwd: &Path) -> Option<String> {
    let git_dir = resolve_git_dir(cwd)?;
    match read_git_head(&git_dir)? {
        GitHead::Branch(name) => resolve_ref(&git_dir, &format!("refs/heads/{name}")),
        GitHead::Detached(sha) => Some(sha),
    }
}

pub fn read_worktree_head_sha(path: &Path) -> Option<String> {
    let contents = fs::read_to_string(path.join(".git")).ok()?;
    let name = contents.trim().strip_prefix("gitdir:")?.trim();
    let pointer = Path::new(name);
    let directory = if pointer.is_absolute() {
        pointer.to_path_buf()
    } else {
        path.join(pointer)
    };
    let git_dir = directory.canonicalize().ok()?;
    match read_git_head(&git_dir)? {
        GitHead::Branch(name) => resolve_ref(&git_dir, &format!("refs/heads/{name}")),
        GitHead::Detached(sha) => Some(sha),
    }
}

pub fn get_remote_url_for_dir(cwd: &Path) -> Option<String> {
    let git_dir = resolve_git_dir(cwd)?;
    crate::git_config_parser::parse_git_config_value(
        &git_dir, "remote", Some("origin"), "url"
    ).filter(|value| !value.is_empty()).or_else(|| {
        let common = get_common_dir(&git_dir)?;
        crate::git_config_parser::parse_git_config_value(
            &common, "remote", Some("origin"), "url"
        )
    })
}

pub fn get_default_branch_for_dir(cwd: &Path) -> String {
    let Some(git_dir) = resolve_git_dir(cwd) else {
        return "main".into();
    };
    let common = get_common_dir(&git_dir).unwrap_or(git_dir);
    if let Some(value) = read_raw_symref(
        &common, "refs/remotes/origin/HEAD", "refs/remotes/origin/"
    ) {
        return value;
    }
    for name in ["main", "master"] {
        if resolve_ref(&common, &format!("refs/remotes/origin/{name}")).is_some() {
            return name.to_owned();
        }
    }
    "main".into()
}

pub fn is_shallow_clone(cwd: &Path) -> bool {
    let Some(git_dir) = resolve_git_dir(cwd) else {
        return false;
    };
    let common = get_common_dir(&git_dir).unwrap_or(git_dir);
    common.join("shallow").exists()
}

pub fn get_worktree_count_from_fs(cwd: &Path) -> usize {
    let Some(git_dir) = resolve_git_dir(cwd) else {
        return 0;
    };
    let common = get_common_dir(&git_dir).unwrap_or(git_dir);
    fs::read_dir(common.join("worktrees"))
        .map(|entries| entries.count() + 1)
        .unwrap_or(1)
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct GitSnapshot {
    pub branch: String,
    pub head: String,
    pub default_branch: String,
    pub remote_url: Option<String>,
    pub shallow: bool,
    pub worktree_count: usize,
}

fn snapshot_uncached(cwd: &Path) -> GitSnapshot {
    let git_dir = resolve_git_dir(cwd);
    let head = git_dir.as_ref().and_then(|path| read_git_head(path));
    let branch = match &head {
        Some(GitHead::Branch(branch)) => branch.clone(),
        _ => "HEAD".into(),
    };
    GitSnapshot {
        head: get_head_for_dir(cwd).unwrap_or_default(),
        branch,
        default_branch: get_default_branch_for_dir(cwd),
        remote_url: get_remote_url_for_dir(cwd),
        shallow: is_shallow_clone(cwd),
        worktree_count: get_worktree_count_from_fs(cwd),
    }
}

/// A fingerprint over the files the original watcher observes. Rechecked on
/// access, so a changed HEAD, ref or shared config never serves stale values.
fn fingerprint(cwd: &Path) -> Vec<u8> {
    let Some(git_dir) = resolve_git_dir(cwd) else {
        return vec![];
    };
    let common = get_common_dir(&git_dir).unwrap_or_else(|| git_dir.clone());
    let branch_ref = match read_git_head(&git_dir) {
        Some(GitHead::Branch(branch)) => Some(common.join("refs/heads").join(branch)),
        _ => None,
    };
    let mut files = vec![
        git_dir.join("HEAD"),
        common.join("config"),
        common.join("packed-refs"),
        common.join("refs/remotes/origin/HEAD"),
        common.join("shallow"),
        common.join("worktrees"),
    ];
    if let Some(path) = branch_ref {
        files.push(path);
    }
    let mut values = Vec::new();
    for path in files {
        values.extend(path.to_string_lossy().as_bytes());
        if path.is_dir() {
            if let Ok(entries) = fs::read_dir(&path) {
                for entry in entries.flatten() {
                    values.extend(entry.file_name().to_string_lossy().as_bytes());
                }
            }
        } else if let Ok(bytes) = fs::read(&path) {
            values.extend(bytes);
        }
        values.push(0);
    }
    values
}

type SnapshotCache = HashMap<PathBuf, (Vec<u8>, GitSnapshot)>;
fn snapshot_cache() -> &'static Mutex<SnapshotCache> {
    static CACHE: OnceLock<Mutex<SnapshotCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}
pub fn reset_git_file_watcher() {
    snapshot_cache().lock().unwrap().clear();
}
pub fn snapshot(cwd: &Path) -> GitSnapshot {
    let cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
    let stamp = fingerprint(&cwd);
    if let Some((cached_stamp, cached)) = snapshot_cache().lock().unwrap().get(&cwd) {
        if cached_stamp == &stamp {
            return cached.clone();
        }
    }
    let actual = snapshot_uncached(&cwd);
    snapshot_cache().lock().unwrap().insert(cwd, (stamp, actual.clone()));
    actual
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn rejects_tampered_branch_and_ref_data() {
        assert!(is_safe_ref_name("feature/a+build.1"));
        for name in ["", ".", "a/./b", "a//b", "-wrong", "a/../b",
            "a;rm", "a\nb", "a@{one", "中文", "/slash"] {
            assert!(!is_safe_ref_name(name), "{name:?}");
        }
        assert!(is_valid_git_sha(&"a".repeat(40)));
        assert!(is_valid_git_sha(&"f".repeat(64)));
        assert!(!is_valid_git_sha(&"A".repeat(40)));
        assert!(!is_valid_git_sha(&"a".repeat(39)));
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("HEAD"), "ref: refs/heads/../../outside\n").unwrap();
        assert!(read_git_head(dir.path()).is_none());
    }

    #[test]
    fn packed_refs_loose_refs_symrefs_and_cycle_detection() {
        let dir = tempfile::tempdir().unwrap();
        let sha1 = "a".repeat(40);
        let sha2 = "b".repeat(40);
        fs::create_dir_all(dir.path().join("refs/heads")).unwrap();
        fs::write(dir.path().join("packed-refs"),
            format!("# packed\n{sha1} refs/heads/main\n^{sha1}\n")).unwrap();
        assert_eq!(resolve_ref(dir.path(), "refs/heads/main"), Some(sha1.clone()));
        fs::write(dir.path().join("refs/heads/main"), &sha2).unwrap();
        assert_eq!(resolve_ref(dir.path(), "refs/heads/main"), Some(sha2.clone()));
        fs::write(dir.path().join("refs/heads/main"), "ref: refs/heads/alias").unwrap();
        fs::write(dir.path().join("refs/heads/alias"), "ref: refs/heads/main").unwrap();
        assert_eq!(resolve_ref(dir.path(), "refs/heads/main"), None);
    }

    #[test]
    fn real_git_checkout_branch_and_cache_invalidations() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        assert!(Command::new("git").args(["init", "-q"])
            .current_dir(root).status().unwrap().success());
        fs::write(root.join("file.txt"), "original").unwrap();
        assert!(Command::new("git").args(["add", "file.txt"])
            .current_dir(root).status().unwrap().success());
        assert!(Command::new("git").args([
            "-c", "user.name=Test", "-c", "user.email=test@example.com",
            "commit", "-qm", "initial"
        ]).current_dir(root).status().unwrap().success());
        let first = snapshot(root);
        assert!(is_valid_git_sha(&first.head));
        assert_eq!(first.worktree_count, 1);
        fs::write(root.join(".git/HEAD"), "ref: refs/heads/newbranch\n").unwrap();
        let second = snapshot(root);
        assert_eq!(second.branch, "newbranch");
        assert_eq!(second.head, "");
        assert_ne!(first, second);
        clear_resolve_git_dir_cache();
    }

    #[test]
    fn real_worktree_pointer_and_common_ref_resolution() {
        let root = tempfile::tempdir().unwrap();
        let repo = root.path();
        let git = repo.join(".git");
        fs::create_dir_all(git.join("refs/heads")).unwrap();
        let sha = "c".repeat(40);
        fs::write(git.join("refs/heads/main"), &sha).unwrap();
        let worktree = root.path().join("branch");
        let private = git.join("worktrees/branch");
        fs::create_dir_all(&private).unwrap();
        fs::create_dir_all(&worktree).unwrap();
        fs::write(private.join("HEAD"), "ref: refs/heads/main\n").unwrap();
        fs::write(private.join("commondir"), "../..\n").unwrap();
        fs::write(worktree.join(".git"), format!("gitdir: {}\n", private.display())).unwrap();
        assert_eq!(get_head_for_dir(&worktree), Some(sha.clone()));
        assert_eq!(read_worktree_head_sha(&worktree), Some(sha));
        assert_eq!(get_worktree_count_from_fs(&worktree), 2);
        fs::write(git.join("shallow"), "").unwrap();
        assert!(is_shallow_clone(&worktree));
        clear_resolve_git_dir_cache();
    }
}
