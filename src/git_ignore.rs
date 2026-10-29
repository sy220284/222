//! Native Git ignore behavior. Git itself resolves local, nested, global
//! and info/exclude rule precedence; no incomplete hand-written matcher.
use anyhow::{bail, Context, Result};
use fs2::FileExt;
use std::{
    fs::{self, OpenOptions},
    io::{Read, Seek, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

/// True only for git check-ignore exit status zero.
pub fn is_path_gitignored(file_path: &str, cwd: &Path) -> bool {
    Command::new("git")
        .arg("check-ignore")
        .arg("--")
        .arg(file_path)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

pub fn global_gitignore_path() -> Result<PathBuf> {
    Ok(dirs::home_dir()
        .context("No home directory available")?
        .join(".config/git/ignore"))
}

/// Explicit user request: add a glob to the global ignore file if it is
/// not already ignored locally or globally. Returns true only on mutation.
pub fn add_file_glob_rule_to_gitignore(filename: &str, cwd: &Path) -> Result<bool> {
    let destination = global_gitignore_path()?;
    add_file_glob_rule_to_gitignore_at(filename, cwd, &destination)
}

fn is_git_repo(cwd: &Path) -> bool {
    let result = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(cwd)
        .stdin(Stdio::null())
        .output();
    result.is_ok_and(|result| {
        result.status.success() && String::from_utf8_lossy(&result.stdout).trim() == "true"
    })
}

fn add_file_glob_rule_to_gitignore_at(
    filename: &str,
    cwd: &Path,
    destination: &Path,
) -> Result<bool> {
    if !is_git_repo(cwd) {
        return Ok(false);
    }
    // Only a single gitignore pattern may be added in one invocation.
    if filename.is_empty() || filename.contains('\n') || filename.contains('\r') {
        bail!("Expected a single nonempty gitignore file pattern");
    }
    let test_path = if filename.ends_with('/') {
        format!("{filename}sample-file.txt")
    } else {
        filename.to_owned()
    };
    if is_path_gitignored(&test_path, cwd) {
        return Ok(false);
    }
    let entry = format!("**/{filename}");
    fs::create_dir_all(
        destination
            .parent()
            .context("Invalid global gitignore path")?,
    )?;
    let mut options = OpenOptions::new();
    options.create(true).read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(destination)?;
    file.lock_exclusive()?;
    let result = (|| -> Result<bool> {
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        // Git patterns are line-oriented: **/foo must not match **/foobar.
        if contents
            .lines()
            .any(|line| line.trim_end_matches('\r') == entry)
        {
            return Ok(false);
        }
        file.seek(std::io::SeekFrom::End(0))?;
        if !contents.is_empty() && !contents.ends_with('\n') {
            file.write_all(b"\n")?;
        }
        file.write_all(entry.as_bytes())?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        Ok(true)
    })();
    let unlocked = FileExt::unlock(&file);
    result.and_then(|changed| {
        unlocked?;
        Ok(changed)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn initialize(root: &Path) {
        let out = Command::new("git")
            .arg("init")
            .arg("-q")
            .current_dir(root)
            .output()
            .unwrap();
        assert!(out.status.success(), "{out:?}");
    }

    #[test]
    fn nested_ignore_and_nonrepository_are_handled_by_git() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_path_gitignored("any.txt", dir.path()));
        let global = dir.path().join("home/.config/git/ignore");
        assert!(!add_file_glob_rule_to_gitignore_at("x", dir.path(), &global).unwrap());
        assert!(!global.exists());
        initialize(dir.path());
        fs::create_dir_all(dir.path().join("nested")).unwrap();
        fs::write(dir.path().join("nested/.gitignore"), "*.log\n").unwrap();
        assert!(is_path_gitignored("nested/a.log", dir.path()));
        assert!(!is_path_gitignored("nested/a.rs", dir.path()));
        assert!(!add_file_glob_rule_to_gitignore_at("nested/a.log", dir.path(), &global).unwrap());
        assert!(!global.exists());
    }

    #[test]
    fn global_rule_addition_matches_exact_lines_and_is_effective_in_git() {
        let dir = tempfile::tempdir().unwrap();
        initialize(dir.path());
        let global = dir.path().join("global-ignore");
        fs::write(&global, "**/foobar\n").unwrap();
        let configured = Command::new("git")
            .args(["config", "core.excludesFile"])
            .arg(&global)
            .current_dir(dir.path())
            .status()
            .unwrap();
        assert!(configured.success());

        // A shorter pattern is not a duplicate of an existing longer pattern.
        assert!(add_file_glob_rule_to_gitignore_at("foo", dir.path(), &global).unwrap());
        assert_eq!(
            fs::read_to_string(&global).unwrap(),
            "**/foobar\n**/foo\n"
        );
        assert!(is_path_gitignored("foo", dir.path()));
        assert!(is_path_gitignored("foobar", dir.path()));
        assert!(!add_file_glob_rule_to_gitignore_at("foo", dir.path(), &global).unwrap());
        assert_eq!(
            fs::read_to_string(&global).unwrap(),
            "**/foobar\n**/foo\n"
        );
    }

    #[test]
    fn global_file_is_created_once_and_directory_rules_use_probe_path() {
        let dir = tempfile::tempdir().unwrap();
        initialize(dir.path());
        let global = dir.path().join("home/.config/git/ignore");
        assert!(add_file_glob_rule_to_gitignore_at("private/", dir.path(), &global).unwrap());
        assert_eq!(fs::read_to_string(&global).unwrap(), "**/private/\n");
        assert!(!add_file_glob_rule_to_gitignore_at("private/", dir.path(), &global).unwrap());
        assert!(add_file_glob_rule_to_gitignore_at("other.txt", dir.path(), &global).unwrap());
        assert_eq!(
            fs::read_to_string(&global).unwrap(),
            "**/private/\n**/other.txt\n"
        );
        assert!(add_file_glob_rule_to_gitignore_at("line\nbad", dir.path(), &global).is_err());
    }
}
