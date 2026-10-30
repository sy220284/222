//! Shared native path behaviors ported from src/utils/path.ts (2.1.88).
//! Authorization remains exclusively in workspace_dirs; normalizing a path
//! never grants access, resolves symlinks, or changes the authorized roots.
use anyhow::{bail, Context, Result};
use std::path::{Component, Path, PathBuf};
use unicode_normalization::UnicodeNormalization;

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                result.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                // Absolute paths cannot traverse beyond their root. For relative
                // inputs retain unresolved leading parents, as Node normalize does.
                if result.file_name().is_some_and(|name| name != "..") {
                    result.pop();
                } else if !result.has_root() {
                    result.push("..");
                }
            }
        }
    }
    if result.as_os_str().is_empty() {
        if path.is_absolute() {
            PathBuf::from(std::path::MAIN_SEPARATOR_STR)
        } else {
            PathBuf::from(".")
        }
    } else {
        result
    }
}

fn nfc(path: PathBuf) -> PathBuf {
    PathBuf::from(path.to_string_lossy().nfc().collect::<String>())
}

/// Expand a user path using the supplied workspace as the relative base.
/// A NUL byte is rejected before any filesystem access. This function does
/// NOT authorize the resulting path; the caller must still check roots.
pub fn expand_path(raw: &str, base: &Path) -> Result<PathBuf> {
    if raw.contains('\0') || base.as_os_str().to_string_lossy().contains('\0') {
        bail!("Path contains null bytes");
    }
    if !base.is_absolute() {
        bail!("Base directory must be absolute: {}", base.display());
    }
    let trimmed = raw.trim();
    let given = if trimmed == "~" || trimmed.starts_with("~/") {
        let home = dirs::home_dir().context("Home directory unavailable")?;
        if trimmed == "~" {
            home
        } else {
            home.join(&trimmed[2..])
        }
    } else if trimmed.is_empty() {
        base.to_path_buf()
    } else {
        #[cfg(windows)]
        let converted = {
            let b = trimmed.as_bytes();
            if b.len() >= 3 && b[0] == b'/' && b[1].is_ascii_alphabetic() && b[2] == b'/' {
                format!("{}:{}", b[1] as char, trimmed[2..].replace('/', "\\"))
            } else {
                trimmed.to_owned()
            }
        };
        #[cfg(not(windows))]
        let converted = trimmed.to_owned();
        PathBuf::from(converted)
    };
    let joined = if given.is_absolute() {
        given
    } else {
        base.join(given)
    };
    Ok(nfc(lexical_normalize(&joined)))
}

/// Keep absolute paths for files outside the user's current workspace.
pub fn to_relative_path(absolute: &Path, cwd: &Path) -> PathBuf {
    let base = lexical_normalize(cwd);
    let value = lexical_normalize(absolute);
    if value == base {
        return PathBuf::new();
    }
    match value.strip_prefix(&base) {
        Ok(relative) => relative.to_path_buf(),
        Err(_) => absolute.to_path_buf(),
    }
}

/// For existing directories return themselves; for files, non-existent paths,
/// and network UNC paths return their parent without probing the remote share.
pub fn get_directory_for_path(raw: &str, base: &Path) -> Result<PathBuf> {
    let path = expand_path(raw, base)?;
    let value = path.to_string_lossy();
    if raw.starts_with("\\\\")
        || raw.starts_with("//")
        || value.starts_with("\\\\")
        || value.starts_with("//")
    {
        return Ok(path.parent().unwrap_or(path.as_path()).to_path_buf());
    }
    if path.is_dir() {
        return Ok(path);
    }
    Ok(path.parent().unwrap_or(path.as_path()).to_path_buf())
}

/// Detect explicit `..` path segments before normalization.
pub fn contains_path_traversal(path: &str) -> bool {
    path.split(['/', '\\']).any(|part| part == "..")
}

/// JSON configuration keys use stable NFC paths and forward slashes on Windows.
pub fn normalize_path_for_config_key(path: &str) -> String {
    let normalized = lexical_normalize(Path::new(path));
    normalized
        .to_string_lossy()
        .replace('\\', "/")
        .nfc()
        .collect()
}

/// Filename-safe project directory key from the portable session-storage
/// source. The original Bun hash differs from Node's fallback, so the stable
/// fallback hash is used here; existing long-path directories must be found
/// through prefix matching when imported from a Bun-based session database.
pub fn sanitize_path(name: &str) -> String {
    const MAX: usize = 200;
    let safe: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    if safe.len() <= MAX {
        return safe;
    }
    // JS djb2Hash uses UTF-16 code units and signed 32-bit wrapping math.
    let mut hash = 0_i32;
    for unit in name.encode_utf16() {
        hash = hash.wrapping_mul(31).wrapping_add(i32::from(unit));
    }
    let mut number = hash.unsigned_abs();
    let mut digits = String::new();
    while number > 0 {
        let digit = number % 36;
        digits.push(char::from_digit(digit, 36).expect("base36 digit"));
        number /= 36;
    }
    if digits.is_empty() {
        digits.push('0');
    }
    format!(
        "{}-{}",
        &safe[..MAX],
        digits.chars().rev().collect::<String>()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_expansion_absolute_relative_home_and_empty() {
        let root = tempfile::tempdir().unwrap();
        let base = root.path();
        assert_eq!(expand_path("", base).unwrap(), base);
        assert_eq!(expand_path("  \t", base).unwrap(), base);
        assert_eq!(expand_path("./a/../b", base).unwrap(), base.join("b"));
        assert_eq!(
            expand_path("~/docs", base).unwrap(),
            dirs::home_dir().unwrap().join("docs")
        );
        assert_eq!(expand_path("~", base).unwrap(), dirs::home_dir().unwrap());
        assert!(expand_path("x\0y", base).is_err());
        assert!(expand_path("x", Path::new("relative")).is_err());
    }

    #[test]
    fn source_nfc_normalization_and_config_key() {
        let dir = tempfile::tempdir().unwrap();
        let expanded = expand_path("cafe\u{301}", dir.path()).unwrap();
        assert_eq!(expanded.file_name().unwrap().to_string_lossy(), "caf\u{e9}");
        assert_eq!(normalize_path_for_config_key("a/./b/../c"), "a/c");
        assert_eq!(
            normalize_path_for_config_key("C:\\some\\path"),
            "C:/some/path"
        );
    }

    #[test]
    fn traversal_raw_input_and_relative_path_boundaries() {
        for p in ["../etc", "a/../b", r"a\..\b", "..", "a/.."] {
            assert!(contains_path_traversal(p), "{p}");
        }
        for p in ["...", "a..", "a/..b", "a/.../b"] {
            assert!(!contains_path_traversal(p), "{p}");
        }
        let root = Path::new("/work");
        assert_eq!(
            to_relative_path(Path::new("/work/src/lib.rs"), root),
            Path::new("src/lib.rs")
        );
        assert_eq!(
            to_relative_path(Path::new("/outside/x"), root),
            Path::new("/outside/x")
        );
        assert_eq!(
            to_relative_path(Path::new("/workshop/x"), root),
            Path::new("/workshop/x")
        );
    }

    #[test]
    fn safe_filename_and_long_path_hash_match_source_fallback() {
        assert_eq!(sanitize_path("/Users/a/test:one"), "-Users-a-test-one");
        assert_eq!(sanitize_path(""), "");
        let long = "A".repeat(240);
        let expected = sanitize_path(&long);
        assert_eq!(expected.len() > 200, true);
        assert_eq!(&expected[..200], "A".repeat(200));
        assert_eq!(sanitize_path(&long), expected);
        assert_ne!(sanitize_path(&(long + "B")), expected);
        assert_eq!(sanitize_path("你好"), "--");
    }

    #[test]
    fn directory_detection_handles_existing_file_new_file_and_directory() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("sub")).unwrap();
        std::fs::write(root.path().join("sub/f.txt"), "x").unwrap();
        assert_eq!(
            get_directory_for_path("sub", root.path()).unwrap(),
            root.path().join("sub")
        );
        assert_eq!(
            get_directory_for_path("sub/f.txt", root.path()).unwrap(),
            root.path().join("sub")
        );
        assert_eq!(
            get_directory_for_path("sub/missing.txt", root.path()).unwrap(),
            root.path().join("sub")
        );
    }
}
