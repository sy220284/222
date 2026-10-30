//! User-approved working directories. Tool calls cannot expand their own access.
//! Hand-ported from commands/add-dir/validation.ts and filesystem.ts (2.1.88).
use anyhow::{bail, Context, Result};
use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

#[derive(Debug, PartialEq, Eq)]
pub enum Validation {
    EmptyPath,
    PathNotFound(PathBuf),
    NotADirectory(PathBuf),
    AlreadyAccessible {
        path: PathBuf,
        existing_root: PathBuf,
    },
    Success(PathBuf),
}

fn expand(raw: &str, cwd: &Path) -> Result<PathBuf> {
    crate::path_support::expand_path(raw, cwd)
}


/** The source validates using the lexical absolute path rather than resolving
 * symlinks. Permission checking is a separate, stricter operation. */
fn comparison_path(path: &Path) -> String {
    let value = path.to_string_lossy().replace('\\', "/");
    let normalized = if let Some(rest) = value.strip_prefix("/private/var/") {
        format!("/var/{rest}")
    } else if value == "/private/tmp" {
        "/tmp".to_owned()
    } else if let Some(rest) = value.strip_prefix("/private/tmp/") {
        format!("/tmp/{rest}")
    } else {
        value.into_owned()
    };
    normalized.to_lowercase()
}

fn path_in_working_path(candidate: &Path, working_root: &Path) -> bool {
    let candidate = comparison_path(candidate);
    let root = comparison_path(working_root);
    if candidate == root {
        return true;
    }
    if root == "/" {
        return candidate.starts_with('/');
    }
    candidate
        .strip_prefix(root.trim_end_matches('/'))
        .is_some_and(|rest| rest.starts_with('/'))
}

pub fn validate(raw: &str, cwd: &Path, primary: &Path, extra: &[PathBuf]) -> Result<Validation> {
    if raw.is_empty() {
        return Ok(Validation::EmptyPath);
    }
    let path = expand(raw, cwd)?;
    let metadata = match fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::NotFound | ErrorKind::PermissionDenied
            ) || error.raw_os_error() == Some(libc::ENOTDIR) =>
        {
            return Ok(Validation::PathNotFound(path));
        }
        Err(error) => return Err(error).with_context(|| format!("Inspect {}", path.display())),
    };
    if !metadata.is_dir() {
        return Ok(Validation::NotADirectory(path));
    }

    // validation.ts returns the expanded lexical path (including a symlink
    // alias), and filesystem.ts compares it to every currently allowed root.
    for allowed in std::iter::once(primary).chain(extra.iter().map(PathBuf::as_path)) {
        let normalized_root = expand(&allowed.to_string_lossy(), cwd)?;
        if path_in_working_path(&path, &normalized_root) {
            return Ok(Validation::AlreadyAccessible {
                path,
                existing_root: allowed.to_path_buf(),
            });
        }
    }
    Ok(Validation::Success(path))
}

/// Canonicalize the nearest existing ancestor to reject symlink escapes,
/// including access to as-yet-uncreated files underneath authorized roots.
pub fn checked_path(raw: &str, primary: &Path, extra: &[PathBuf]) -> Result<PathBuf> {
    if raw.is_empty() {
        bail!("Empty file path");
    }
    let path = expand(raw, primary)?;
    let mut ancestor = path.as_path();
    while fs::symlink_metadata(ancestor).is_err_and(|e| e.kind() == ErrorKind::NotFound) {
        ancestor = ancestor.parent().context("No existing ancestor")?;
    }
    let actual = ancestor.canonicalize()?;
    if !std::iter::once(primary)
        .chain(extra.iter().map(PathBuf::as_path))
        .any(|root| actual.starts_with(root))
    {
        bail!(
            "Path outside authorized working directories: {}",
            path.display()
        );
    }
    Ok(path)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn directory_validation_has_distinct_outcomes() {
        let dir = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("child")).unwrap();
        fs::write(dir.path().join("file"), "x").unwrap();
        let root = dir.path().canonicalize().unwrap();
        let external = other.path().canonicalize().unwrap();
        assert_eq!(
            validate("", &root, &root, &[]).unwrap(),
            Validation::EmptyPath
        );
        assert!(matches!(
            validate("missing", &root, &root, &[]).unwrap(),
            Validation::PathNotFound(_)
        ));
        assert!(matches!(
            validate("file", &root, &root, &[]).unwrap(),
            Validation::NotADirectory(_)
        ));
        assert!(matches!(
            validate("child/", &root, &root, &[]).unwrap(),
            Validation::AlreadyAccessible { .. }
        ));
        assert_eq!(
            validate(external.to_str().unwrap(), &root, &root, &[]).unwrap(),
            Validation::Success(external.clone())
        );
        assert!(matches!(
            validate(
                external.to_str().unwrap(),
                &root,
                &root,
                &[external.clone()]
            )
            .unwrap(),
            Validation::AlreadyAccessible { .. }
        ));
    }
    #[test]
    fn lexical_source_semantics_and_directory_boundary() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        fs::create_dir(root.join("child")).unwrap();
        let sibling = root.with_file_name(format!(
            "{}2", root.file_name().unwrap().to_string_lossy()
        ));
        fs::create_dir(&sibling).unwrap();
        assert_eq!(
            validate(sibling.to_str().unwrap(), &root, &root, &[]).unwrap(),
            Validation::Success(sibling.clone())
        );
        assert_eq!(
            validate("child/../", &root, &root, &[]).unwrap(),
            Validation::AlreadyAccessible {
                path: root.clone(),
                existing_root: root.clone()
            }
        );
        fs::remove_dir(&sibling).unwrap();

        #[cfg(unix)]
        {
            let outside = tempfile::tempdir().unwrap();
            std::os::unix::fs::symlink(outside.path(), root.join("alias")).unwrap();
            assert_eq!(
                validate("alias/", &root, &root, &[]).unwrap(),
                Validation::AlreadyAccessible {
                    path: root.join("alias"),
                    existing_root: root.clone()
                }
            );
        }
    }

    #[test]
    fn authorized_access_and_symlink_escapes() {
        let dir = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let forbidden = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let extra = other.path().canonicalize().unwrap();
        assert_eq!(
            checked_path(extra.join("new").to_str().unwrap(), &root, &[extra.clone()]).unwrap(),
            extra.join("new")
        );
        assert!(checked_path(forbidden.path().to_str().unwrap(), &root, &[extra]).is_err());
        assert!(checked_path("../out", &root, &[]).is_err());
        assert!(checked_path("a/../b", &root, &[]).unwrap().ends_with("b"));
        assert!(expand("invalid\0path", &root).is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(forbidden.path(), root.join("link")).unwrap();
            assert!(checked_path("link/secret", &root, &[]).is_err());
        }
    }
}
