//! Native marketplace input parser from 2.1.88
//! `src/utils/plugins/parseMarketplaceInput.ts`. Sources are parsed without
//! activating or downloading untrusted remote plugins.
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Component, Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "lowercase")]
pub enum MarketplaceSource {
    Git {
        url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        r#ref: Option<String>,
    },
    Github {
        repo: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        r#ref: Option<String>,
    },
    Url {
        url: String,
    },
    File {
        path: PathBuf,
    },
    Directory {
        path: PathBuf,
    },
}

fn split_fragment(value: &str) -> (&str, Option<String>) {
    if let Some((url, suffix)) = value.split_once('#') {
        if !suffix.is_empty() {
            return (url, Some(suffix.to_owned()));
        }
    }
    (value, None)
}

fn local_path(raw: &str, cwd: &Path, home: &Path, windows: bool) -> PathBuf {
    let expanded = if let Some(tail) = raw.strip_prefix('~') {
        PathBuf::from(format!("{}{}", home.display(), tail))
    } else if windows && raw.starts_with('\\') {
        PathBuf::from(raw)
    } else {
        PathBuf::from(raw)
    };
    let full = if expanded.is_absolute() {
        expanded
    } else {
        cwd.join(expanded)
    };
    // Node's path.resolve normalizes dots lexically; it does not dereference
    // symbolic links. `metadata` below follows symlinks as fs.stat does.
    let mut parts: Vec<std::ffi::OsString> = Vec::new();
    let mut prefix = PathBuf::new();
    for component in full.components() {
        match component {
            Component::Prefix(p) => prefix.push(p.as_os_str()),
            Component::RootDir => {
                prefix.push(component.as_os_str());
                parts.clear();
            }
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop();
            }
            Component::Normal(p) => parts.push(p.to_owned()),
        }
    }
    for p in parts {
        prefix.push(p);
    }
    prefix
}

pub fn parse_marketplace_input(
    input: &str,
    cwd: &Path,
    home: &Path,
    windows: bool,
) -> Result<Option<MarketplaceSource>, String> {
    let trimmed = input.trim();
    // Any non-empty valid SSH username, not only `git`, is supported.
    if let Some((user, rest)) = trimmed.split_once('@') {
        if !user.is_empty()
            && user
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            if let Some((host, repo)) = rest.split_once(':') {
                if !host.is_empty() && !repo.is_empty() {
                    let (url, reference) = split_fragment(trimmed);
                    return Ok(Some(MarketplaceSource::Git {
                        url: url.to_owned(),
                        r#ref: reference,
                    }));
                }
            }
        }
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        let (url, reference) = split_fragment(trimmed);
        if url.ends_with(".git") || url.contains("/_git/") {
            return Ok(Some(MarketplaceSource::Git {
                url: url.to_owned(),
                r#ref: reference,
            }));
        }
        if let Ok(parsed) = reqwest::Url::parse(url) {
            if matches!(parsed.host_str(), Some("github.com" | "www.github.com")) {
                let segments = parsed
                    .path_segments()
                    .map(|s| s.collect::<Vec<_>>())
                    .unwrap_or_default();
                if segments.len() >= 2 && !segments[0].is_empty() && !segments[1].is_empty() {
                    return Ok(Some(MarketplaceSource::Git {
                        url: format!("{url}.git"),
                        r#ref: reference,
                    }));
                }
            }
        }
        return Ok(Some(MarketplaceSource::Url {
            url: url.to_owned(),
        }));
    }
    let is_win_path = windows
        && (trimmed.starts_with(".\\")
            || trimmed.starts_with("..\\")
            || (trimmed.as_bytes().len() > 2
                && trimmed.as_bytes()[0].is_ascii_alphabetic()
                && trimmed.as_bytes()[1] == b':'
                && matches!(trimmed.as_bytes()[2], b'/' | b'\\')));
    if trimmed.starts_with("./")
        || trimmed.starts_with("../")
        || trimmed.starts_with('/')
        || trimmed.starts_with('~')
        || is_win_path
    {
        let path = local_path(trimmed, cwd, home, windows);
        let stats = fs::metadata(&path).map_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                format!("Path does not exist: {}", path.display())
            } else {
                format!("Cannot access path: {} ({e})", path.display())
            }
        })?;
        if stats.is_file() {
            if path.to_string_lossy().ends_with(".json") {
                return Ok(Some(MarketplaceSource::File { path }));
            }
            return Err(format!(
                "File path must point to a .json file (marketplace.json), but got: {}",
                path.display()
            ));
        }
        if stats.is_dir() {
            return Ok(Some(MarketplaceSource::Directory { path }));
        }
        return Err(format!(
            "Path is neither a file nor a directory: {}",
            path.display()
        ));
    }
    if trimmed.contains('/') && !trimmed.starts_with('@') {
        if trimmed.contains(':') {
            return Ok(None);
        }
        let pos = trimmed
            .char_indices()
            .find(|(_, c)| matches!(c, '#' | '@'))
            .map(|(p, _)| p);
        let (repo, reference) = match pos {
            Some(i) if i + 1 < trimmed.len() => (&trimmed[..i], Some(trimmed[i + 1..].to_owned())),
            _ => (trimmed, None),
        };
        return Ok(Some(MarketplaceSource::Github {
            repo: repo.to_owned(),
            r#ref: reference,
        }));
    }
    Ok(None)
}

pub fn parse_in_directory(input: &str, cwd: &Path) -> Result<Option<MarketplaceSource>, String> {
    let home = dirs::home_dir().unwrap_or_else(|| cwd.to_owned());
    parse_marketplace_input(input, cwd, &home, cfg!(windows))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ssh_remote_refs_accept_enterprise_usernames_and_hosts() {
        assert_eq!(
            parse_marketplace_input(
                "org-123456@github.com:org/repo.git#dev",
                Path::new("/tmp"),
                Path::new("/home/x"),
                false
            )
            .unwrap(),
            Some(MarketplaceSource::Git {
                url: "org-123456@github.com:org/repo.git".into(),
                r#ref: Some("dev".into())
            })
        );
        assert_eq!(
            parse_marketplace_input(
                "deploy@192.168.1.2:team/repo",
                Path::new("/tmp"),
                Path::new("/home/x"),
                false
            )
            .unwrap(),
            Some(MarketplaceSource::Git {
                url: "deploy@192.168.1.2:team/repo".into(),
                r#ref: None
            })
        );
    }
    #[test]
    fn azure_git_vs_json_url_and_github_http_are_distinguished() {
        let parse = |url| {
            parse_marketplace_input(url, Path::new("/tmp"), Path::new("/home/x"), false)
                .unwrap()
                .unwrap()
        };
        assert_eq!(
            parse("https://dev.azure.com/org/proj/_git/repo#main"),
            MarketplaceSource::Git {
                url: "https://dev.azure.com/org/proj/_git/repo".into(),
                r#ref: Some("main".into())
            }
        );
        assert_eq!(
            parse("https://github.com/a/b#v1"),
            MarketplaceSource::Git {
                url: "https://github.com/a/b.git".into(),
                r#ref: Some("v1".into())
            }
        );
        assert_eq!(
            parse("https://example.org/marketplace.json#anchor"),
            MarketplaceSource::Url {
                url: "https://example.org/marketplace.json".into()
            }
        );
        assert_eq!(
            parse("http://git.example.org/path.git"),
            MarketplaceSource::Git {
                url: "http://git.example.org/path.git".into(),
                r#ref: None
            }
        );
    }
    #[test]
    fn github_shorthand_at_and_hash_refs_and_invalid_inputs() {
        let parse =
            |s| parse_marketplace_input(s, Path::new("/tmp"), Path::new("/home/x"), false).unwrap();
        assert_eq!(
            parse("owner/repo@main"),
            Some(MarketplaceSource::Github {
                repo: "owner/repo".into(),
                r#ref: Some("main".into())
            })
        );
        assert_eq!(
            parse("owner/repo#tag"),
            Some(MarketplaceSource::Github {
                repo: "owner/repo".into(),
                r#ref: Some("tag".into())
            })
        );
        assert_eq!(parse("owner:repo/invalid"), None);
        assert_eq!(parse("a-package"), None);
        assert_eq!(parse("@scoped/package"), None);
    }
    #[test]
    fn local_paths_stat_file_vs_directory_and_errors() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::create_dir(root.join("catalog")).unwrap();
        fs::write(root.join("catalog.json"), "{}").unwrap();
        fs::write(root.join("market.txt"), "{}").unwrap();
        let parse = |s: &str| parse_marketplace_input(s, root, root, false);
        assert_eq!(
            parse("./catalog").unwrap(),
            Some(MarketplaceSource::Directory {
                path: root.join("catalog")
            })
        );
        assert_eq!(
            parse("./catalog.json").unwrap(),
            Some(MarketplaceSource::File {
                path: root.join("catalog.json")
            })
        );
        assert!(parse("./market.txt")
            .unwrap_err()
            .contains("must point to a .json file"));
        assert!(parse("./missing")
            .unwrap_err()
            .contains("Path does not exist"));
        assert_eq!(
            parse("~/catalog").unwrap(),
            Some(MarketplaceSource::Directory {
                path: root.join("catalog")
            })
        );
    }
    #[test]
    fn windows_paths_only_apply_on_windows() {
        let cwd = Path::new("/tmp");
        let home = Path::new("/home/x");
        assert_eq!(
            parse_marketplace_input(".\\marketplace", cwd, home, false).unwrap(),
            None
        );
        assert!(parse_marketplace_input(".\\marketplace", cwd, home, true).is_err());
        assert_eq!(
            parse_marketplace_input("C:\\work\\marketplace", cwd, home, false).unwrap(),
            None
        );
    }
}
