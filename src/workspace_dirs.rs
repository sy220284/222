//! User-approved working directories. Tool calls cannot expand their own access.
use anyhow::{bail, Context, Result};
use std::{
    fs,
    io::ErrorKind,
    path::{Component, Path, PathBuf},
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
    if raw.contains('\0') {
        bail!("Directory path contains a NUL byte");
    }
    let input = raw.trim();
    let given = if input == "~" || input.starts_with("~/") {
        let home = dirs::home_dir().context("Home directory unavailable")?;
        if input == "~" {
            home
        } else {
            home.join(&input[2..])
        }
    } else {
        PathBuf::from(input)
    };
    let absolute = if given.is_absolute() {
        given
    } else {
        cwd.join(given)
    };
    let mut normalized = PathBuf::new();
    for part in absolute.components() {
        match part {
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(part.as_os_str())
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    bail!("Directory path escapes filesystem root");
                }
            }
        }
    }
    Ok(normalized)
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
            return Ok(Validation::PathNotFound(path))
        }
        Err(error) => return Err(error).with_context(|| format!("Inspect {}", path.display())),
    };
    if !metadata.is_dir() {
        return Ok(Validation::NotADirectory(path));
    }
    let resolved = path.canonicalize()?;
    for allowed in std::iter::once(primary).chain(extra.iter().map(PathBuf::as_path)) {
        if resolved.starts_with(allowed) {
            return Ok(Validation::AlreadyAccessible {
                path: resolved,
                existing_root: allowed.to_path_buf(),
            });
        }
    }
    Ok(Validation::Success(resolved))
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
