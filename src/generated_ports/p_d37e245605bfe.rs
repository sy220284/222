//! Native Rust port derived from original: src/utils/suggestions/directoryCompletion.ts
//!
//! Path completion decision core: parsePartialPath (empty→base,
//! trailing separator→no prefix, else dirname+basename), the
//! `matchedOnNameOnly` flag when the typed prefix matches the last
//! segment, hidden-entry exclusion, and the LRU cache contract
//! (500 entries, 5-minute TTL). Filesystem scanning is injected.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde_json::{json, Value};

pub const CACHE_SIZE: usize = 500;
pub const CACHE_TTL_MS: u64 = 5 * 60 * 1000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedPath {
    pub directory: String,
    pub prefix: String,
}

/// `parsePartialPath` over normalized POSIX-style paths.
pub fn parse_partial_path(partial_path: &str, base_path: Option<&str>, cwd: &str) -> ParsedPath {
    if partial_path.is_empty() {
        return ParsedPath {
            directory: base_path.unwrap_or(cwd).to_string(),
            prefix: String::new(),
        };
    }
    if partial_path.ends_with('/') {
        return ParsedPath {
            directory: partial_path.to_string(),
            prefix: String::new(),
        };
    }
    match partial_path.rsplit_once('/') {
        Some((dir, prefix)) => ParsedPath {
            directory: dir.to_string(),
            prefix: prefix.to_string(),
        },
        None => ParsedPath {
            directory: base_path.unwrap_or(cwd).to_string(),
            prefix: partial_path.to_string(),
        },
    }
}

/// `matchedOnNameOnly`: typed prefix matches the entry's own name.
pub fn matched_on_name_only(prefix: &str, entry_name: &str) -> bool {
    !prefix.is_empty() && entry_name.starts_with(prefix)
}

/// Filter entries: directories only (unless include_files), hidden
/// excluded (unless include_hidden), prefix-matched, capped.
pub fn complete_entries(
    entries: &[(String, bool, bool)],
    parsed: &ParsedPath,
    include_files: bool,
    include_hidden: bool,
    max_results: usize,
) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for (name, is_dir, hidden) in entries {
        if !*is_dir && !include_files {
            continue;
        }
        if *hidden && !include_hidden {
            continue;
        }
        if !matched_on_name_only(&parsed.prefix, name) {
            continue;
        }
        let path = if parsed.directory.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", parsed.directory.trim_end_matches('/'), name)
        };
        out.push((name.clone(), path));
        if out.len() >= max_results {
            break;
        }
    }
    out
}

/// Registry view shim.
pub fn parsePartialPath(args: &[Value]) -> Value {
    let p = args.first().and_then(|v| v.as_str()).unwrap_or("");
    let base = args.get(1).and_then(|v| v.as_str());
    let parsed = parse_partial_path(p, base, ".");
    json!({"directory": parsed.directory, "prefix": parsed.prefix})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_and_trailing_separator() {
        assert_eq!(
            parse_partial_path("", Some("/base"), "/cwd"),
            ParsedPath {
                directory: "/base".into(),
                prefix: String::new()
            }
        );
        assert_eq!(
            parse_partial_path("", None, "/cwd"),
            ParsedPath {
                directory: "/cwd".into(),
                prefix: String::new()
            }
        );
        assert_eq!(
            parse_partial_path("src/", None, "/cwd"),
            ParsedPath {
                directory: "src/".into(),
                prefix: String::new()
            }
        );
    }

    #[test]
    fn parse_splits_directory_and_prefix() {
        assert_eq!(
            parse_partial_path("src/com", None, "/cwd"),
            ParsedPath {
                directory: "src".into(),
                prefix: "com".into()
            }
        );
        assert_eq!(
            parse_partial_path("com", None, "/cwd"),
            ParsedPath {
                directory: "/cwd".into(),
                prefix: "com".into()
            }
        );
    }

    #[test]
    fn name_only_matching_and_hidden_rules() {
        assert!(matched_on_name_only("com", "components"));
        assert!(!matched_on_name_only("", "components"));
        let entries = vec![
            (".git".to_string(), true, true),
            ("components".to_string(), true, false),
            ("main.rs".to_string(), false, false),
        ];
        let parsed = ParsedPath {
            directory: "src".into(),
            prefix: "c".into(),
        };
        let out = complete_entries(&entries, &parsed, false, false, 10);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].0, "components");
        let parsed2 = ParsedPath {
            directory: "src".into(),
            prefix: "m".into(),
        };
        let out2 = complete_entries(&entries, &parsed2, true, false, 10);
        assert_eq!(out2.len(), 1);
        assert_eq!(out2[0].1, "src/main.rs");
    }

    #[test]
    fn cache_contract_values() {
        assert_eq!(CACHE_SIZE, 500);
        assert_eq!(CACHE_TTL_MS, 300_000);
    }
}

/// Registry-view wrapper: path-like token detection (contains / or
/// starts with ./ or ../ or ~/).
pub fn isPathLikeToken(args: &[Value]) -> Value {
    let token = args.first().and_then(|v| v.as_str()).unwrap_or("");
    let like = token.contains('/')
        || token.starts_with("./")
        || token.starts_with("../")
        || token.starts_with("~/");
    json!(like)
}
