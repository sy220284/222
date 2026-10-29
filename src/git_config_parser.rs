//! Source-compatible Git config reader. Section/key lookups ignore case;
//! a quoted subsection is case-sensitive.
use std::{fs, path::Path};

/// Return the first matching key in repeated config sections.
pub fn parse_config_string(
    config: &str,
    section: &str,
    subsection: Option<&str>,
    key: &str,
) -> Option<String> {
    let mut matching = false;
    for line in config.split('\n') {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }
        if trimmed.starts_with('[') {
            matching = matches_section_header(trimmed, section, subsection);
            continue;
        }
        if matching {
            if let Some((found, value)) = parse_key_value(trimmed) {
                if found.eq_ignore_ascii_case(key) {
                    return Some(value);
                }
            }
        }
    }
    None
}

/// Missing or unreadable configuration returns None.
pub fn parse_git_config_value(
    git_dir: &Path,
    section: &str,
    subsection: Option<&str>,
    key: &str,
) -> Option<String> {
    let raw = fs::read_to_string(git_dir.join("config")).ok()?;
    parse_config_string(&raw, section, subsection, key)
}

fn parse_key_value(line: &str) -> Option<(&str, String)> {
    let bytes = line.as_bytes();
    let mut pos = 0;
    while pos < bytes.len() && (bytes[pos].is_ascii_alphanumeric() || bytes[pos] == b'-') {
        pos += 1;
    }
    if pos == 0 {
        return None;
    }
    let key = &line[..pos];
    while pos < bytes.len() && matches!(bytes[pos], b' ' | b'\t') {
        pos += 1;
    }
    if bytes.get(pos) != Some(&b'=') {
        return None;
    }
    pos += 1;
    while pos < bytes.len() && matches!(bytes[pos], b' ' | b'\t') {
        pos += 1;
    }
    Some((key, parse_value(&line[pos..])))
}

fn parse_value(raw: &str) -> String {
    let mut characters = raw.chars().peekable();
    let mut result = String::new();
    let mut in_quote = false;
    while let Some(ch) = characters.next() {
        if !in_quote && matches!(ch, '#' | ';') {
            break;
        }
        if ch == '"' {
            in_quote = !in_quote;
            continue;
        }
        if ch == '\\' {
            if let Some(&next) = characters.peek() {
                if in_quote {
                    characters.next();
                    result.push(match next {
                        'n' => '\n',
                        't' => '\t',
                        'b' => '\u{0008}',
                        _ => next,
                    });
                    continue;
                }
                if next == '\\' {
                    characters.next();
                    result.push('\\');
                    continue;
                }
            }
        }
        result.push(ch);
    }
    if !in_quote {
        result.trim_end_matches([' ', '\t']).to_owned()
    } else {
        result
    }
}

fn matches_section_header(line: &str, section: &str, subsection: Option<&str>) -> bool {
    let mut chars = line.chars().peekable();
    if chars.next() != Some('[') {
        return false;
    }
    let mut found_section = String::new();
    while let Some(&next) = chars.peek() {
        if matches!(next, ']' | ' ' | '\t' | '"') {
            break;
        }
        found_section.push(chars.next().unwrap());
    }
    if !found_section.eq_ignore_ascii_case(section) {
        return false;
    }
    let Some(expected) = subsection else {
        return chars.peek() == Some(&']');
    };
    while chars.peek().is_some_and(|ch| matches!(ch, ' ' | '\t')) {
        chars.next();
    }
    if chars.next() != Some('"') {
        return false;
    }
    let mut found = String::new();
    let mut closed = false;
    while let Some(ch) = chars.next() {
        if ch == '"' {
            closed = true;
            break;
        }
        if ch == '\\' {
            if let Some(next) = chars.next() {
                found.push(next);
                continue;
            }
        }
        found.push(ch);
    }
    closed && chars.next() == Some(']') && found == expected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_sections_first_value_and_case_rules() {
        let config = r#"
# comment
[remote "Origin"]
url = wrong
[REMOTE "origin"]
URL = ssh://git@host.example.com/owner/repo.git
url = later
[core]
hooksPath = /tmp/hooks
[remote "origin"]
url = too-late
"#;
        assert_eq!(
            parse_config_string(config, "remote", Some("origin"), "url").as_deref(),
            Some("ssh://git@host.example.com/owner/repo.git")
        );
        assert_eq!(
            parse_config_string(config, "CORE", None, "hooksPath").as_deref(),
            Some("/tmp/hooks")
        );
        assert_eq!(parse_config_string(config, "remote", None, "url"), None);
        assert_eq!(
            parse_config_string(config, "remote", Some("ORIGIN"), "url"),
            None
        );
    }

    #[test]
    fn quotes_escapes_comments_unicode_and_invalid_keys() {
        let config = "[core]\npath = \"a\\n\\tb\\b\\\\c\\\"d\" # trailing\n\
                      other = /one\\path  ; comment\n\
                      quoted = \"你好 # world\"\n\
                      badKey\n\
                      [remote \"escaped\\\\\\\"quote\"]\n\
                      url = important\n";
        assert_eq!(
            parse_config_string(config, "core", None, "path").as_deref(),
            Some("a\n\tb\u{0008}\\c\"d")
        );
        assert_eq!(
            parse_config_string(config, "core", None, "other").as_deref(),
            Some("/one\\path")
        );
        assert_eq!(
            parse_config_string(config, "core", None, "quoted").as_deref(),
            Some("你好 # world")
        );
        assert_eq!(
            parse_config_string(config, "remote", Some("escaped\\\"quote"), "url").as_deref(),
            Some("important")
        );
        assert_eq!(parse_config_string(config, "core", None, "badKey"), None);
    }

    #[test]
    fn reads_real_config_and_handles_missing_config() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            parse_git_config_value(dir.path(), "core", None, "hooksPath"),
            None
        );
        fs::write(
            dir.path().join("config"),
            "[core]\n hooksPath = \"../hooks dir\"\n",
        )
        .unwrap();
        assert_eq!(
            parse_git_config_value(dir.path(), "core", None, "hookspath").as_deref(),
            Some("../hooks dir")
        );
    }
}
