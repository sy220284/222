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
