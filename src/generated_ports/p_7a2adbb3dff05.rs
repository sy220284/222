//! Native Rust port derived from original: src/utils/debugFilter.ts
//!
//! Debug filter parsing and category matching for `--debug=pattern` output.
//! The original wraps `parseDebugFilter` in lodash `memoize`, which means a
//! second call with a different argument returns the first result. The Rust
//! port is a pure function (callers hold their own value), so repeated calls
//! with different patterns behave correctly; the memoized cache identity is
//! not preserved. Recorded as a remaining difference in the port matrix.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde::{Deserialize, Serialize};

/// Lowercase filter categories with include/exclude semantics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct DebugFilter {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub is_exclusive: bool,
}

/// Parse a debug filter string into a filter configuration.
///
/// Examples:
/// - `"api,hooks"` -> include only api and hooks categories
/// - `"!1p,!file"` -> exclude logging and file categories
/// - `None`/empty -> no filtering (show all)
pub fn parse_debug_filter(filter_string: Option<&str>) -> Option<DebugFilter> {
    let raw = filter_string?;
    if raw.trim().is_empty() {
        return None;
    }

    let filters: Vec<String> = raw
        .split(',')
        .map(|f| f.trim().to_owned())
        .filter(|f| !f.is_empty())
        .collect();

    if filters.is_empty() {
        return None;
    }

    let has_exclusive = filters.iter().any(|f| f.starts_with('!'));
    let has_inclusive = filters.iter().any(|f| !f.starts_with('!'));

    if has_exclusive && has_inclusive {
        // Mixed inclusive/exclusive filters are an error case: show all
        // messages (null filter) exactly like the original.
        return None;
    }

    let clean: Vec<String> = filters
        .iter()
        .map(|f| f.trim_start_matches('!').to_lowercase())
        .collect();

    let (include, exclude) = if has_exclusive {
        (Vec::new(), clean)
    } else {
        (clean, Vec::new())
    };

    Some(DebugFilter {
        include,
        exclude,
        is_exclusive: has_exclusive,
    })
}

/// Extract debug categories from a message.
///
/// Supported patterns (lowercase output for case-insensitive matching):
/// - `"category: message"` -> `["category"]`
/// - `"[CATEGORY] message"` -> `["category"]`
/// - `"MCP server \"name\": message"` -> `["mcp", "name"]`
/// - `"[ANT-ONLY] 1P event: timer_started"` -> `["ant-only", "1p"]`
pub fn extract_debug_categories(message: &str) -> Vec<String> {
    use regex::Regex;

    let mut categories: Vec<String> = Vec::new();

    // Pattern 3: MCP server "servername" - checked first to avoid false positives.
    let mcp = Regex::new(r#"^MCP server ["']([^"']+)["']"#).ok();
    if let Some(re) = &mcp {
        if let Some(caps) = re.captures(message) {
            if let Some(name) = caps.get(1) {
                categories.push("mcp".into());
                categories.push(name.as_str().to_lowercase());
            }
        }
    }

    if !categories.iter().any(|c| c == "mcp") {
        // Pattern 1: "category: message" (simple prefix) - only if not MCP pattern.
        let prefix = Regex::new(r"^([^:\[]+):").ok();
        if let Some(re) = &prefix {
            if let Some(caps) = re.captures(message) {
                if let Some(cat) = caps.get(1) {
                    categories.push(cat.as_str().trim().to_lowercase());
                }
            }
        }
    }

    // Pattern 2: [CATEGORY] at the start.
    let bracket = Regex::new(r"^\[([^\]]+)]").ok();
    if let Some(re) = &bracket {
        if let Some(caps) = re.captures(message) {
            if let Some(cat) = caps.get(1) {
                categories.push(cat.as_str().trim().to_lowercase());
            }
        }
    }

    // Pattern 4: additional categories inside the message body.
    if message.to_lowercase().contains("1p event:") {
        categories.push("1p".into());
    }

    // Pattern 5: secondary categories after the first pattern.
    let secondary = Regex::new(r":\s*([^:]+?)(?:\s+(?:type|mode|status|event))?:").ok();
    if let Some(re) = &secondary {
        if let Some(caps) = re.captures(message) {
            if let Some(sec) = caps.get(1) {
                let s = sec.as_str().trim().to_lowercase();
                // Only add if it's a reasonable category name (not too long, no spaces).
                if s.len() < 30 && !s.contains(' ') {
                    categories.push(s);
                }
            }
        }
    }

    // Remove duplicates while preserving first-seen order.
    let mut seen = std::collections::HashSet::new();
    categories.retain(|c| seen.insert(c.clone()));
    categories
}

/// Check whether a debug message should be shown based on extracted
/// categories and a parsed filter.
pub fn should_show_debug_categories(categories: &[String], filter: Option<&DebugFilter>) -> bool {
    // No filter means show everything.
    let Some(filter) = filter else {
        return true;
    };

    if categories.is_empty() {
        // Uncategorized messages are excluded in both modes: exclusive mode
        // excludes them by default for security, inclusive mode requires a
        // category match.
        return false;
    }

    if filter.is_exclusive {
        // Exclusive mode: show if none of the categories are in the exclude list.
        !categories.iter().any(|cat| filter.exclude.contains(cat))
    } else {
        // Inclusive mode: show if any of the categories are in the include list.
        categories.iter().any(|cat| filter.include.contains(cat))
    }
}

/// Main entry: combine extraction and filtering.
pub fn should_show_debug_message(message: &str, filter: Option<&DebugFilter>) -> bool {
    // Fast path: no filter means show everything.
    let Some(filter) = filter else {
        return true;
    };
    let categories = extract_debug_categories(message);
    should_show_debug_categories(&categories, Some(filter))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_filter_modes() {
        assert!(parse_debug_filter(None).is_none());
        assert!(parse_debug_filter(Some("  ")).is_none());

        let inc = parse_debug_filter(Some("api, hooks")).unwrap();
        assert_eq!(inc.include, vec!["api", "hooks"]);
        assert_eq!(inc.exclude, Vec::<String>::new());
        assert!(!inc.is_exclusive);

        let exc = parse_debug_filter(Some("!1p, !file")).unwrap();
        assert_eq!(exc.exclude, vec!["1p", "file"]);
        assert!(exc.include.is_empty());
        assert!(exc.is_exclusive);

        // Mixed filters are rejected -> null -> show all.
        assert!(parse_debug_filter(Some("api,!1p")).is_none());
    }

    #[test]
    fn extract_categories_matches_original_patterns() {
        assert_eq!(
            extract_debug_categories("api: something happened"),
            vec!["api"]
        );
        assert_eq!(
            extract_debug_categories("[CATEGORY] message"),
            vec!["category"]
        );
        assert_eq!(
            extract_debug_categories("MCP server \"Files\": connected"),
            vec!["mcp", "files"]
        );
        assert_eq!(
            extract_debug_categories("[ANT-ONLY] 1P event: timer_started"),
            vec!["ant-only", "1p"]
        );
        // Secondary category extraction.
        assert_eq!(
            extract_debug_categories("AutoUpdaterWrapper: Installation type: development"),
            vec!["autoupdaterwrapper", "installation"]
        );
        // Long or spaced secondaries are dropped.
        assert!(!extract_debug_categories("a: this has spaces: b")
            .contains(&"this has spaces".to_string()));
    }

    #[test]
    fn filter_modes_show_and_hide() {
        let inc = parse_debug_filter(Some("api")).unwrap();
        assert!(should_show_debug_message("api: hit", Some(&inc)));
        assert!(!should_show_debug_message("hooks: miss", Some(&inc)));
        assert!(!should_show_debug_message("uncategorized", Some(&inc)));

        let exc = parse_debug_filter(Some("!1p")).unwrap();
        assert!(should_show_debug_message("api: shown", Some(&exc)));
        assert!(!should_show_debug_message("1P event: hidden", Some(&exc)));

        // No filter -> show all.
        assert!(should_show_debug_message("anything", None));
    }
}
