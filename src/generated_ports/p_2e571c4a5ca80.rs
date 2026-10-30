//! Native Rust port derived from original: src/utils/frontmatterParser.ts
//!
//! Markdown frontmatter parsing: `---` delimiters, special-character
//! value quoting (the YAML_SPECIAL_CHARS table and the ': ' nuance are
//! verbatim), and the typed accessor set for known keys. The upstream
//! `parseYaml` dependency has no Rust equivalent, so this port ships a
//! lightweight key-value subset (flat `key: value` lines, quoted and
//! list values) — nested YAML is a recorded remaining difference.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

/// Characters that require quoting in YAML values; `: ` is matched as
/// the two-char sequence (times like 12:34 and URLs stay unquoted).
pub fn value_needs_quoting(value: &str) -> bool {
    for c in [
        '{', '}', '[', ']', '*', '&', '#', '!', '|', '>', '%', '@', '`',
    ] {
        if value.contains(c) {
            return true;
        }
    }
    value.contains(": ")
}

/// `quoteProblematicValues`: quote simple `key: value` lines whose value
/// contains special YAML characters; list items, block scalars and
/// already-quoted lines pass through untouched.
pub fn quote_problematic_values(frontmatter_text: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for line in frontmatter_text.split('\n') {
        if let Some((key, value)) = split_kv(line) {
            let value = value.trim();
            let already_quoted = value.starts_with('"') || value.starts_with('\'');
            if !already_quoted && value_needs_quoting(value) {
                out.push(format!("{key}: \"{value}\""));
                continue;
            }
        }
        out.push(line.to_string());
    }
    out.join("\n")
}

/// Split `key: value` (top-level only: no leading space, not a list item).
fn split_kv(line: &str) -> Option<(&str, &str)> {
    if line.starts_with(' ') || line.starts_with('-') || line.starts_with('#') {
        return None;
    }
    line.split_once(": ")
}

/// Extract frontmatter between leading `---` delimiters.
pub fn parse_frontmatter(content: &str) -> (std::collections::HashMap<String, String>, String) {
    let mut map = std::collections::HashMap::new();
    let trimmed = content.trim_start();
    let Some(rest) = trimmed.strip_prefix("---\n") else {
        return (map, content.to_string());
    };
    let Some(end) = rest.find("\n---") else {
        return (map, content.to_string());
    };
    let fm = &rest[..end];
    let body = rest[end + 4..].trim_start_matches('\n');
    for line in quote_problematic_values(fm).split('\n') {
        if let Some((k, v)) = split_kv(line) {
            let v = v.trim();
            let v = v
                .strip_prefix('"')
                .unwrap_or(v)
                .strip_suffix('"')
                .unwrap_or(v);
            if !v.is_empty() && v != "null" {
                map.insert(k.trim().to_string(), v.to_string());
            }
        }
    }
    (map, body.to_string())
}

/// Known key accessors with list splitting (original semantics).
pub fn allowed_tools_list(fm: &std::collections::HashMap<String, String>) -> Vec<String> {
    fm.get("allowed-tools")
        .map(|v| {
            v.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_values_get_quoted() {
        // Braces and the ": " nuance from the original comment.
        assert!(value_needs_quoting("**/*.{ts,tsx}"));
        assert!(!value_needs_quoting("https://example.com"));
        assert!(value_needs_quoting("a: b"));
    }

    #[test]
    fn quoting_leaves_lists_and_times_alone() {
        let fm = "paths:\n  - src/**/*.ts\nversion: 12:34\nwhen_to_use: Use it {now}";
        let out = quote_problematic_values(fm);
        assert!(out.contains("- src/**/*.ts")); // list item untouched
        assert!(out.contains("version: 12:34")); // time without space-colon untouched
        assert!(out.contains("when_to_use: \"Use it {now}\"")); // brace forces quotes
    }

    #[test]
    fn parse_extracts_frontmatter_and_body() {
        let doc = "---\ndescription: Run tests\nallowed-tools: Bash, Read\n---\n\nBody here";
        let (fm, body) = parse_frontmatter(doc);
        assert_eq!(fm.get("description").map(String::as_str), Some("Run tests"));
        assert_eq!(allowed_tools_list(&fm), vec!["Bash", "Read"]);
        assert_eq!(body, "Body here");
    }

    #[test]
    fn no_frontmatter_returns_body() {
        let (fm, body) = parse_frontmatter("Just text");
        assert!(fm.is_empty());
        assert_eq!(body, "Just text");
    }
}

/// Registry-view wrapper: boolean frontmatter access (args[0]=doc,
/// args[1]=key, args[2]=default).
pub fn parseBooleanFrontmatter(args: &[serde_json::Value]) -> serde_json::Value {
    let doc = args.first().and_then(|v| v.as_str()).unwrap_or("");
    let key = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
    let default = args.get(2).and_then(|v| v.as_bool()).unwrap_or(false);
    let (fm, _) = parse_frontmatter(doc);
    serde_json::json!(fm.get(key).map(|v| v == "true").unwrap_or(default))
}
