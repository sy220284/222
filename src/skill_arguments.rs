//! Skill/command prompt substitution from utils/argumentSubstitution.ts.
//! Values are parsed as shell arguments, never interpreted as commands.
use regex::{Captures, Regex};
use serde_json::Value;
pub fn parse_arguments(raw: &str) -> Vec<String> {
    if raw.trim().is_empty() {
        return Vec::new();
    }
    match crate::shell_quote::parse_words(raw) {
        Ok(parts) => parts,
        Err(_) => raw.split_whitespace().map(str::to_owned).collect(),
    }
}
pub fn parse_argument_names(value: Option<&Value>) -> Vec<String> {
    let valid = |s: &str| !s.trim().is_empty() && !s.chars().all(|c| c.is_ascii_digit());
    match value {
        Some(Value::String(v)) => v
            .split_whitespace()
            .filter(|s| valid(s))
            .map(str::to_owned)
            .collect(),
        Some(Value::Array(v)) => v
            .iter()
            .filter_map(Value::as_str)
            .filter(|s| valid(s))
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}
pub fn progressive_hint(names: &[String], typed: &[String]) -> Option<String> {
    let rest = names.get(typed.len()..)?;
    if rest.is_empty() {
        None
    } else {
        Some(
            rest.iter()
                .map(|n| format!("[{n}]"))
                .collect::<Vec<_>>()
                .join(" "),
        )
    }
}
pub fn substitute_arguments(
    content: &str,
    args: Option<&str>,
    append: bool,
    argument_names: &[String],
) -> String {
    let Some(args) = args else {
        return content.to_owned();
    };
    let parts = parse_arguments(args);
    let mut text = content.to_owned();
    for (i, name) in argument_names.iter().enumerate() {
        if name.is_empty() {
            continue;
        }
        let re = Regex::new(&format!(r"\${}([\[A-Za-z0-9_])?", regex::escape(name))).unwrap();
        text = re
            .replace_all(&text, |capture: &Captures<'_>| {
                if capture.get(1).is_some() {
                    capture[0].to_owned()
                } else {
                    parts.get(i).cloned().unwrap_or_default()
                }
            })
            .into_owned();
    }
    let indexed = Regex::new(r"\$ARGUMENTS\[(\d+)\]").unwrap();
    text = indexed
        .replace_all(&text, |capture: &Captures<'_>| {
            capture[1]
                .parse::<usize>()
                .ok()
                .and_then(|i| parts.get(i))
                .cloned()
                .unwrap_or_default()
        })
        .into_owned();
    let shorthand = Regex::new(r"\$(\d+)([A-Za-z0-9_])?").unwrap();
    text = shorthand
        .replace_all(&text, |capture: &Captures<'_>| {
            if capture.get(2).is_some() {
                capture[0].to_owned()
            } else {
                capture[1]
                    .parse::<usize>()
                    .ok()
                    .and_then(|i| parts.get(i))
                    .cloned()
                    .unwrap_or_default()
            }
        })
        .into_owned();
    text = text.replace("$ARGUMENTS", args);
    if text == content && append && !args.is_empty() {
        text.push_str("\n\nARGUMENTS: ");
        text.push_str(args);
    }
    text
}

/// The source skill loader parses YAML frontmatter separately and only
/// supplies the Markdown body to the model. Keep the original file available
/// for reading its argument declarations, but do not inject metadata as
/// instructions. An unterminated frontmatter header is preserved as content.
pub fn markdown_body(content: &str) -> &str {
    let mut lines = content.split_inclusive('\n');
    let Some(first) = lines.next() else {
        return content;
    };
    if first.trim() != "---" {
        return content;
    }
    let mut offset = first.len();
    for line in lines {
        offset += line.len();
        if line.trim() == "---" {
            return &content[offset..];
        }
    }
    content
}

/// Read an explicit top-level YAML boolean from a closed frontmatter header.
/// Original parseBooleanFrontmatter treats only the boolean/string true as true;
/// a missing field remains None so each caller can choose its own default.
pub fn frontmatter_boolean(content: &str, key: &str) -> Option<bool> {
    let mut lines = content.lines();
    if lines.next().map(str::trim) != Some("---") {
        return None;
    }
    let mut found = None;
    let mut closed = false;
    for line in lines {
        if line.trim() == "---" {
            closed = true;
            break;
        }
        let Some(value) = line.strip_prefix(key).and_then(|v| v.strip_prefix(':')) else {
            continue;
        };
        let value = value.trim().split(" #").next().unwrap_or("").trim();
        let value = value.trim_matches(|c| c == '\'' || c == '"');
        found = Some(value == "true");
    }
    if closed { found } else { None }
}

/// Return a top-level string field from a closed YAML frontmatter header.
/// The SDK command catalog reads description and argument-hint from the same
/// source file as the interactive slash command. Nested metadata must not
/// override these user-visible fields.
pub fn frontmatter_text(content: &str, key: &str) -> Option<String> {
    let mut lines = content.lines();
    if lines.next().map(str::trim) != Some("---") {
        return None;
    }
    // YAML block scalars are terminated by indentation, not by the next
    // arbitrary line. Parse within the complete, closed frontmatter so a
    // following "argument-hint" or "model" field cannot erase description.
    let mut header = Vec::new();
    let mut closed = false;
    for line in lines {
        if line.trim() == "---" {
            closed = true;
            break;
        }
        header.push(line);
    }
    if !closed {
        return None;
    }
    let mut found = None;
    let mut index = 0;
    while index < header.len() {
        let line = header[index];
        index += 1;
        if line.starts_with(' ') || line.starts_with('\t') {
            continue;
        }
        let Some(raw) = line.strip_prefix(key).and_then(|tail| tail.strip_prefix(':')) else {
            continue;
        };
        let raw = raw.trim();
        let marker = raw.split_whitespace().next().unwrap_or("");
        let block_marker = marker.chars().next().filter(|ch| matches!(ch, '|' | '>'));
        if let Some(kind) = block_marker.filter(|_| marker.chars().skip(1)
            .all(|ch| matches!(ch, '+' | '-' | '1'..='9'))) {
            let mut raw_lines = Vec::new();
            while index < header.len() {
                let next = header[index];
                if !next.trim().is_empty()
                    && !next.starts_with(' ') && !next.starts_with('\t') {
                    break;
                }
                raw_lines.push(next);
                index += 1;
            }
            let indent = raw_lines.iter()
                .filter(|line| !line.trim().is_empty())
                .map(|line| line.len() - line.trim_start_matches([' ', '\t']).len())
                .min().unwrap_or(0);
            let block = raw_lines.into_iter().map(|line| {
                if line.trim().is_empty() {
                    String::new()
                } else {
                    line.get(indent..).unwrap_or(line).to_owned()
                }
            }).collect::<Vec<_>>();
            let mut value = String::new();
            for (i, line) in block.iter().enumerate() {
                if i > 0 {
                    let folded = kind == '>' && !line.is_empty()
                        && !block[i - 1].is_empty()
                        && !line.starts_with(' ') && !block[i - 1].starts_with(' ');
                    value.push(if folded { ' ' } else { '\n' });
                }
                value.push_str(line);
            }
            // YAML clip is the default; strip and keep are explicit.
            if marker.contains('-') {
                value = value.trim_end_matches('\n').to_owned();
            } else if !marker.contains('+') && !value.is_empty() {
                value = format!("{}\n", value.trim_end_matches('\n'));
            }
            found = Some(value);
            continue;
        }
        let value = if raw.starts_with('"') && raw.ends_with('"') && raw.len() >= 2 {
            serde_json::from_str::<String>(raw)
                .unwrap_or_else(|_| raw[1..raw.len() - 1].to_owned())
        } else if raw.starts_with('\'') && raw.ends_with('\'') && raw.len() >= 2 {
            raw[1..raw.len() - 1].replace("''", "'")
        } else {
            raw.split(" #").next().unwrap_or("").trim().to_owned()
        };
        found = Some(value);
    }
    found
}

/// SDK metadata comes from the real command/skill Markdown, with the same
/// explicit frontmatter precedence as the original command loaders.
pub fn command_metadata(content: &str, fallback: &str) -> (String, String) {
    let description = frontmatter_text(content, "description")
        .filter(|value| !value.trim().is_empty())
        .or_else(|| markdown_body(content).lines()
            .map(str::trim)
            .find(|line| !line.is_empty() && !line.starts_with("<!--"))
            .map(|line| line.trim_start_matches('#').trim().to_owned()))
        .unwrap_or_else(|| fallback.to_owned());
    let hint = frontmatter_text(content, "argument-hint").unwrap_or_default();
    (description, hint)
}

/// Reads optional arguments frontmatter declaration from SKILL.md.
/// Supports whitespace-separated string, inline arrays, YAML multiline arrays.
pub fn frontmatter_argument_names(content: &str) -> Vec<String> {
    let mut lines = content.lines();
    if lines.next().map(str::trim) != Some("---") {
        return Vec::new();
    }
    let mut is_list = false;
    let mut list = Vec::new();
    for line in lines.take(120) {
        if line.trim() == "---" {
            break;
        }
        if is_list {
            // YAML sequences may contain blank lines and comments between
            // entries. The source frontmatter parser handles those without
            // terminating the argument list.
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if let Some(value) = trimmed.strip_prefix("- ") {
                list.push(
                    value
                        .trim()
                        .trim_matches(|c| c == '\'' || c == '"')
                        .to_owned(),
                );
                continue;
            }
            break;
        }
        // Only a top-level YAML field declares skill argument names.
        // A nested metadata.arguments field must not shadow it.
        let Some(value) = line.strip_prefix("arguments:") else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            is_list = true;
            continue;
        }
        if value.starts_with('[') && value.ends_with(']') {
            return value[1..value.len() - 1]
                .split(',')
                .map(|s| s.trim().trim_matches(|c| c == '\'' || c == '"').to_owned())
                .filter(|s| !s.is_empty() && !s.chars().all(|c| c.is_ascii_digit()))
                .collect();
        }
        return parse_argument_names(Some(&Value::String(
            value.trim_matches(|c| c == '\'' || c == '"').to_owned(),
        )));
    }
    parse_argument_names(Some(&serde_json::json!(list)))
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn frontmatter_flags_respect_top_level_and_defaults() {
        assert_eq!(frontmatter_boolean("---\ndisable-model-invocation: true\n---\nbody",
            "disable-model-invocation"), Some(true));
        assert_eq!(frontmatter_boolean("---\nmetadata:\n  disable-model-invocation: true\n---\nbody",
            "disable-model-invocation"), None);
        assert_eq!(frontmatter_boolean("---\nuser-invocable: false\n---\nbody",
            "user-invocable"), Some(false));
        assert_eq!(frontmatter_boolean("body", "user-invocable"), None);
    }
    #[test]
    fn shell_argument_quotes_operators_and_literal_variables() {
        assert_eq!(
            parse_arguments(r#"foo "two words" 'three words' bar\ baz"#),
            vec!["foo", "two words", "three words", "bar baz"]
        );
        assert_eq!(
            parse_arguments("foo $HOME | cat"),
            vec!["foo", "$HOME", "cat"]
        );
        assert_eq!(parse_arguments("foo 'unclosed"), vec!["foo", "'unclosed"]);
    }
    #[test]
    fn named_indexed_raw_and_append_behave_in_order() {
        let names = vec!["topic".into(), "path".into()];
        assert_eq!(
            substitute_arguments(
                "$topic $path $ARGUMENTS[0] $ARGUMENTS[1] $0 $1 $ARGUMENTS",
                Some(r#"alpha "two words""#),
                true,
                &names
            ),
            r#"alpha two words alpha two words alpha two words alpha "two words""#
        );
        assert_eq!(
            substitute_arguments("$topicX $topic[0] $0end", Some("hi"), true, &names),
            "$topicX $topic[0] $0end\n\nARGUMENTS: hi"
        );
        assert_eq!(
            substitute_arguments("plain", Some("hi"), true, &[]),
            "plain\n\nARGUMENTS: hi"
        );
        assert_eq!(substitute_arguments("plain", Some(""), true, &[]), "plain");
        assert_eq!(substitute_arguments("$ARGUMENTS", Some(""), true, &[]), "");
        assert_eq!(
            substitute_arguments("$ARGUMENTS", None, true, &[]),
            "$ARGUMENTS"
        );
    }
    #[test]
    fn frontmatter_names_and_progressive_hint() {
        let input="---\nname: build\narguments:\n  - component\n  - platform\n---\nUse $component for $platform";
        let names = frontmatter_argument_names(input);
        assert_eq!(names, vec!["component", "platform"]);
        assert_eq!(
            progressive_hint(&names, &["a".into()]),
            Some("[platform]".into())
        );
        assert_eq!(progressive_hint(&names, &["a".into(), "b".into()]), None);
        assert_eq!(
            parse_argument_names(Some(&json!(["foo", "123", "", "bar"]))),
            vec!["foo", "bar"]
        );
    }
}
