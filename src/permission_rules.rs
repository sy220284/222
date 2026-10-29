//! Provider-neutral permission rule parser and shell-rule matcher.
//! Based on src/utils/permissions/permissionRuleParser.ts and shellRuleMatching.ts.
use anyhow::{bail, Result};
use regex::RegexBuilder;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleValue {
    pub tool_name: String,
    pub rule_content: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Behavior {
    Allow,
    Deny,
    Ask,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuleSetting {
    pub behavior: String,
    pub rule: String,
}
#[derive(Clone, Debug)]
pub struct Rule {
    pub behavior: Behavior,
    pub value: RuleValue,
}

pub fn normalize_legacy_tool(name: &str) -> &str {
    match name {
        "Task" => "Agent",
        "KillShell" => "TaskStop",
        "AgentOutputTool" | "BashOutputTool" => "TaskOutput",
        other => other,
    }
}
pub fn legacy_names(name: &str) -> Vec<&'static str> {
    match name {
        "Agent" => vec!["Task"],
        "TaskStop" => vec!["KillShell"],
        "TaskOutput" => vec!["AgentOutputTool", "BashOutputTool"],
        _ => Vec::new(),
    }
}
pub fn escape_content(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
}
pub fn unescape_content(s: &str) -> String {
    s.replace("\\(", "(")
        .replace("\\)", ")")
        .replace("\\\\", "\\")
}
fn first_unescaped(s: &str, target: char) -> Option<usize> {
    s.match_indices(target)
        .find(|(index, _)| {
            s[..*index]
                .bytes()
                .rev()
                .take_while(|c| *c == b'\\')
                .count()
                % 2
                == 0
        })
        .map(|(index, _)| index)
}
fn last_unescaped(s: &str, target: char) -> Option<usize> {
    s.match_indices(target)
        .rev()
        .find(|(index, _)| {
            s[..*index]
                .bytes()
                .rev()
                .take_while(|c| *c == b'\\')
                .count()
                % 2
                == 0
        })
        .map(|(index, _)| index)
}
pub fn parse_rule(s: &str) -> RuleValue {
    let tool_only = || RuleValue {
        tool_name: normalize_legacy_tool(s).into(),
        rule_content: None,
    };
    let Some(start) = first_unescaped(s, '(') else {
        return tool_only();
    };
    let Some(end) = last_unescaped(s, ')') else {
        return tool_only();
    };
    if end <= start || end != s.len() - 1 || start == 0 {
        return tool_only();
    }
    let tool_name = normalize_legacy_tool(&s[..start]).to_owned();
    let body = &s[start + 1..end];
    if body.is_empty() || body == "*" {
        return RuleValue {
            tool_name,
            rule_content: None,
        };
    }
    RuleValue {
        tool_name,
        rule_content: Some(unescape_content(body)),
    }
}
/// Validate persisted permission rules before applying them. The source
/// settings loader filters invalid entries individually, keeping valid rules
/// from the same settings file (settings/permissionValidation.ts).
pub fn valid_stored_rule(raw: &str) -> bool {
    if raw.trim().is_empty() {
        return false;
    }
    let bytes = raw.as_bytes();
    let mut opens = 0usize;
    let mut closes = 0usize;
    for (index, ch) in bytes.iter().enumerate() {
        if *ch != b'(' && *ch != b')' {
            continue;
        }
        let escaped = bytes[..index]
            .iter()
            .rev()
            .take_while(|byte| **byte == b'\\')
            .count()
            % 2
            != 0;
        if !escaped {
            if *ch == b'(' {
                opens += 1;
                if bytes.get(index + 1) == Some(&b')') {
                    return false;
                }
            } else {
                closes += 1;
            }
        }
    }
    if opens != closes {
        return false;
    }
    let value = parse_rule(raw);
    let name = value.tool_name.as_str();
    if name.is_empty() || name.chars().next().is_some_and(|ch| ch.is_lowercase()) {
        // MCP tool names are the only lowercase persisted tool identifiers.
        if !name.starts_with("mcp__") {
            return false;
        }
    }
    if let Some(mcp_name) = name.strip_prefix("mcp__") {
        let server = mcp_name.split("__").next().unwrap_or("");
        return !server.is_empty() && opens == 0;
    }
    let Some(content) = value.rule_content.as_deref() else {
        return true;
    };
    match name {
        "Bash" => {
            if (content.contains(":*") && !content.ends_with(":*")) || content == ":*" {
                return false;
            }
        }
        "Read" | "Write" | "Edit" | "Glob" | "NotebookRead" | "NotebookEdit" => {
            if content.contains(":*") {
                return false;
            }
            if content.contains('*')
                && !content.starts_with('*')
                && !content.ends_with('*')
                && !content.contains("**")
                && !content.contains("/*")
                && !content.contains("*.")
                && !content.contains("*)")
            {
                return false;
            }
        }
        "WebFetch" => {
            if content.contains("://")
                || content.starts_with("http")
                || !content.starts_with("domain:")
            {
                return false;
            }
        }
        "WebSearch" if content.contains('*') || content.contains('?') => return false,
        _ => {}
    }
    true
}

pub fn format_rule(value: &RuleValue) -> String {
    match value.rule_content.as_deref().filter(|s| !s.is_empty()) {
        Some(content) => format!("{}({})", value.tool_name, escape_content(content)),
        None => value.tool_name.clone(),
    }
}
pub fn extract_prefix(rule: &str) -> Option<&str> {
    rule.strip_suffix(":*").filter(|prefix| !prefix.is_empty())
}
pub fn has_wildcards(rule: &str) -> bool {
    if rule.ends_with(":*") {
        return false;
    }
    rule.match_indices('*').any(|(index, _)| {
        rule[..index]
            .bytes()
            .rev()
            .take_while(|c| *c == b'\\')
            .count()
            % 2
            == 0
    })
}
pub fn match_wildcard(pattern: &str, command: &str, case_insensitive: bool) -> Result<bool> {
    let p = pattern.trim();
    let mut regex = String::from("^");
    let mut chars = p.chars().peekable();
    let mut unescaped_stars = 0usize;
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.peek().copied() {
                Some('*') | Some('\\') => {
                    let next = chars.next().expect("peek succeeded");
                    regex.push_str(&regex::escape(&next.to_string()));
                    continue;
                }
                _ => {}
            }
        }
        if ch == '*' {
            regex.push_str(".*");
            unescaped_stars += 1;
        } else {
            regex.push_str(&regex::escape(&ch.to_string()));
        }
    }
    if unescaped_stars == 1 && regex.ends_with(" .*") {
        regex.truncate(regex.len() - 3);
        regex.push_str("( .*)?");
    }
    regex.push('$');
    let compiled = RegexBuilder::new(&regex)
        .case_insensitive(case_insensitive)
        .dot_matches_new_line(true)
        .build()?;
    Ok(compiled.is_match(command))
}

pub fn suggestion_for_exact_command(tool_name: &str, command: &str) -> Value {
    serde_json::json!([{
        "type":"addRules",
        "rules":[{"toolName":tool_name,"ruleContent":command}],
        "behavior":"allow",
        "destination":"localSettings"
    }])
}

pub fn suggestion_for_prefix(tool_name: &str, prefix: &str) -> Value {
    serde_json::json!([{
        "type":"addRules",
        "rules":[{"toolName":tool_name,"ruleContent":format!("{prefix}:*")}],
        "behavior":"allow",
        "destination":"localSettings"
    }])
}

pub fn matches_content(rule: &str, command: &str) -> Result<bool> {
    if let Some(prefix) = extract_prefix(rule) {
        return Ok(command == prefix
            || command
                .strip_prefix(prefix)
                .is_some_and(|rest| rest.starts_with(' ')));
    }
    if has_wildcards(rule) {
        return match_wildcard(rule, command, false);
    }
    Ok(rule == command)
}
pub fn canonical_tool(name: &str) -> &str {
    match name {
        "run_command" | "Bash" => "Bash",
        "task_stop" | "TaskStop" | "KillShell" => "TaskStop",
        "task_output" | "TaskOutput" | "AgentOutputTool" | "BashOutputTool" => "TaskOutput",
        "spawn_agent" | "spawn_team" | "Agent" | "Task" => "Agent",
        "read_file" | "Read" => "Read",
        "write_file" | "Write" => "Write",
        "edit_file" | "Edit" => "Edit",
        "web_fetch" | "WebFetch" => "WebFetch",
        other => normalize_legacy_tool(other),
    }
}
pub fn compile_rules(settings: &[RuleSetting]) -> Result<Vec<Rule>> {
    settings
        .iter()
        .map(|setting| {
            let behavior = match setting.behavior.as_str() {
                "allow" => Behavior::Allow,
                "deny" => Behavior::Deny,
                "ask" => Behavior::Ask,
                other => bail!("Unknown permission behavior: {other}"),
            };
            let value = parse_rule(&setting.rule);
            if value.tool_name.trim().is_empty() {
                bail!("Empty permission tool name")
            }
            if setting.rule.len() > 4096 {
                bail!("Permission rule too long")
            }
            Ok(Rule { behavior, value })
        })
        .collect()
}
/// Shell rules are evaluated against individual commands after splitting
/// control operators. A prefix/wildcard cannot grant a whole compound command.
/// Unanalyzable constructs require a literal whole-command permission.
#[derive(Debug)]
struct ShellSegments<'a> {
    commands: Vec<&'a str>,
    analyzable: bool,
}
fn shell_segments(command: &str) -> ShellSegments<'_> {
    let bytes = command.as_bytes();
    let mut commands = Vec::new();
    let mut start = 0usize;
    let mut quote = None;
    let mut escaped = false;
    let mut analyzable = command.len() <= 10_000;
    for (index, ch) in command.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            } else if q == '"' && ch == '\\' {
                escaped = true;
            } else if q == '"'
                && ((ch as u32) == 96 || (ch == '$' && bytes.get(index + 1) == Some(&b'(')))
            {
                analyzable = false;
            }
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '\'' | '"' => quote = Some(ch),
            '(' | ')' | '{' | '}' | '<' | '>' => analyzable = false,
            '$' if bytes.get(index + 1) == Some(&b'(') => analyzable = false,
            '#' => analyzable = false,
            c if (c as u32) == 96 => analyzable = false,
            '&' | '|' | ';' | '\n' => {
                if ch == '&'
                    && bytes.get(index.wrapping_sub(1)) != Some(&b'&')
                    && bytes.get(index + 1) != Some(&b'&')
                {
                    analyzable = false;
                }
                let part = command[start..index].trim();
                if !part.is_empty() {
                    commands.push(part);
                }
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    let tail = command[start..].trim();
    if !tail.is_empty() {
        commands.push(tail);
    }
    if quote.is_some() || escaped || commands.len() > 50 {
        analyzable = false;
    }
    for part in &commands {
        let mut words = part.split_whitespace();
        let first = words.next().unwrap_or("");
        if matches!(first, "eval" | "source" | ".")
            || (matches!(
                first,
                "sh" | "bash"
                    | "zsh"
                    | "dash"
                    | "ksh"
                    | "fish"
                    | "csh"
                    | "tcsh"
                    | "pwsh"
                    | "powershell"
                    | "cmd"
            ) && words.any(|word| word == "-c" || word == "/C"))
        {
            analyzable = false;
        }
    }
    ShellSegments {
        commands,
        analyzable,
    }
}

fn shell_policy(rules: &[Rule], command: &str) -> Result<Option<Behavior>> {
    let parts = shell_segments(command);
    let mut allow_whole = false;
    let mut allow_each = vec![false; parts.commands.len()];
    let mut ask = false;
    for rule in rules {
        if canonical_tool(&rule.value.tool_name) != "Bash" {
            continue;
        }
        let pattern = rule.value.rule_content.as_deref();
        let whole = match pattern {
            None => true,
            Some(p) => matches_content(p, command)?,
        };
        let submatches = parts
            .commands
            .iter()
            .map(|part| match pattern {
                None => Ok(true),
                Some(p) => matches_content(p, part),
            })
            .collect::<Result<Vec<_>>>()?;
        if !whole && !submatches.iter().any(|v| *v) {
            continue;
        }
        match rule.behavior {
            Behavior::Deny => return Ok(Some(Behavior::Deny)),
            Behavior::Ask => ask = true,
            Behavior::Allow => {
                let exact_full = pattern
                    .is_some_and(|p| extract_prefix(p).is_none() && !has_wildcards(p) && whole);
                if pattern.is_none() || exact_full {
                    allow_whole = true;
                }
                if parts.analyzable {
                    for (index, matched) in submatches.iter().enumerate() {
                        allow_each[index] |= *matched;
                    }
                }
            }
        }
    }
    if ask {
        return Ok(Some(Behavior::Ask));
    }
    if allow_whole || (parts.analyzable && !allow_each.is_empty() && allow_each.iter().all(|v| *v))
    {
        Ok(Some(Behavior::Allow))
    } else {
        Ok(None)
    }
}

/// The original permissions.ts::toolMatchesRule permits a permission on
/// "mcp__server" or "mcp__server__*" to target every tool of that server.
/// A rule with parenthesized contents is never a whole-tool rule.
fn mcp_server_rule_matches(rule: &str, tool: &str) -> bool {
    fn parts(name: &str) -> Option<(&str, Option<&str>)> {
        let body = name.strip_prefix("mcp__")?;
        let mut split = body.splitn(2, "__");
        let server = split.next().filter(|v| !v.is_empty())?;
        Some((server, split.next()))
    }
    let (rule_server, rule_tool) = match parts(rule) {
        Some(x) => x,
        None => return false,
    };
    let (actual_server, actual_tool) = match parts(tool) {
        Some(x) => x,
        None => return false,
    };
    actual_tool.is_some() && actual_server == rule_server && matches!(rule_tool, None | Some("*"))
}

/// Filesystem permission patterns use path globs, not shell command
/// wildcards. The original filesystem.ts::matchingRuleForInput applies Edit
/// path rules to all file-mutating tools and matches resolved symlink targets.
fn file_pattern_matches(pattern: &str, args: &Value) -> Result<bool> {
    let pattern = pattern.strip_prefix("./").unwrap_or(pattern);
    let glob = globset::Glob::new(pattern)?.compile_matcher();
    let directory = pattern.strip_suffix("/**");
    let matches = |path: &str| {
        let normalized = path.replace('\\', "/");
        glob.is_match(&normalized) || directory.is_some_and(|root| normalized == root)
    };
    Ok(args
        .get("paths")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .any(&matches)
        || args
            .get("path")
            .and_then(Value::as_str)
            .is_some_and(matches))
}

pub fn policy_for(rules: &[Rule], tool: &str, args: &Value) -> Result<Option<Behavior>> {
    let name = canonical_tool(tool);
    if name == "Bash" {
        if let Some(command) = args.get("command").and_then(Value::as_str) {
            return shell_policy(rules, command);
        }
    }
    let mut allow = false;
    let mut ask = false;
    for rule in rules {
        let rule_name = canonical_tool(&rule.value.tool_name);
        // Edit(path) is the original shared filesystem edit permission:
        // it also governs Write, but a tool-wide Edit rule is not a
        // blanket grant for every other tool.
        let shared_edit_path_rule = matches!(name, "Write" | "Edit" | "notebook_edit")
            && rule_name == "Edit"
            && rule.value.rule_content.is_some();
        if rule_name != name
            && !shared_edit_path_rule
            && (rule.value.rule_content.is_some()
                || !mcp_server_rule_matches(&rule.value.tool_name, name))
        {
            continue;
        }
        let matched = match rule.value.rule_content.as_deref() {
            None => true,
            Some(pattern) => {
                let argument = match name {
                    "Bash" => args.get("command").and_then(Value::as_str),
                    "Read" | "Write" | "Edit" => args.get("path").and_then(Value::as_str),
                    "TaskStop" => args
                        .get("task_id")
                        .or_else(|| args.get("shell_id"))
                        .and_then(Value::as_str),
                    "Agent" if tool == "spawn_agent" || tool == "Agent" => Some(
                        args.get("subagent_type")
                            .or_else(|| args.get("agent_type"))
                            .and_then(Value::as_str)
                            .unwrap_or("general"),
                    ),
                    _ => None,
                };
                if matches!(name, "Read" | "Write" | "Edit" | "notebook_edit") {
                    file_pattern_matches(pattern, args)?
                } else if name == "WebFetch" {
                    // WebFetchTool.ts constructs domain:<URL.hostname> and
                    // looks up that exact content key (not a prefix/wildcard).
                    match args
                        .get("url")
                        .and_then(Value::as_str)
                        .and_then(|url| reqwest::Url::parse(url).ok())
                        .and_then(|url| url.host_str().map(str::to_owned))
                    {
                        Some(host) => pattern == format!("domain:{host}"),
                        None => false,
                    }
                } else if name == "Agent" {
                    // permissions.ts::getDenyRuleForAgent matches agent type
                    // exactly; shell wildcard/prefix syntax is not used.
                    argument == Some(pattern)
                } else {
                    match argument {
                        Some(argument) => matches_content(pattern, argument)?,
                        None => false,
                    }
                }
            }
        };
        if !matched {
            continue;
        }
        match rule.behavior {
            Behavior::Deny => return Ok(Some(Behavior::Deny)),
            Behavior::Ask => ask = true,
            Behavior::Allow => allow = true,
        }
    }
    if ask {
        Ok(Some(Behavior::Ask))
    } else if allow {
        Ok(Some(Behavior::Allow))
    } else {
        Ok(None)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn agent_type_rules_are_exact_and_do_not_match_unrelated_team_calls() {
        // permissions.ts::getDenyRuleForAgent compares ruleContent with
        // agentType; the generic Agent tool-wide rule remains independent.
        let rules = compile_rules(&[RuleSetting {
            behavior: "deny".into(),
            rule: "Agent(Explore)".into(),
        }]).unwrap();
        assert_eq!(
            policy_for(&rules, "spawn_agent", &json!({
                "prompt":"inspect", "subagent_type":"Explore"
            })).unwrap(),
            Some(Behavior::Deny)
        );
        assert_eq!(
            policy_for(&rules, "spawn_agent", &json!({
                "prompt":"inspect", "subagent_type":"Explorer"
            })).unwrap(),
            None
        );
        assert_eq!(
            policy_for(&rules, "spawn_agent", &json!({"prompt":"inspect"})).unwrap(),
            None
        );
        assert_eq!(
            policy_for(&rules, "spawn_team", &json!({
                "tasks":[{"name":"Explore","prompt":"inspect"}]
            })).unwrap(),
            None
        );
        let defaults = compile_rules(&[RuleSetting {
            behavior: "deny".into(),
            rule: "Agent(general)".into(),
        }]).unwrap();
        assert_eq!(
            policy_for(&defaults, "spawn_agent", &json!({"prompt":"inspect"})).unwrap(),
            Some(Behavior::Deny)
        );
    }

    #[test]
    fn edit_path_rules_cover_write_and_resolved_targets() {
        // filesystem.ts::getPatternsByRoot('edit') uses Edit rules for
        // every file-mutating tool; path matching is gitignore-style.
        let rules = compile_rules(&[
            RuleSetting {
                behavior: "deny".into(),
                rule: "Edit(secrets/**)".into(),
            },
            RuleSetting {
                behavior: "allow".into(),
                rule: "Write(alias.txt)".into(),
            },
            RuleSetting {
                behavior: "allow".into(),
                rule: "Edit(public/**)".into(),
            },
        ])
        .unwrap();
        assert_eq!(
            policy_for(
                &rules,
                "write_file",
                &json!({
                    "path":"alias.txt",
                    "paths":["alias.txt", "secrets/key.txt"]
                })
            )
            .unwrap(),
            Some(Behavior::Deny)
        );
        assert_eq!(
            policy_for(
                &rules,
                "write_file",
                &json!({
                    "path":"public/new.txt", "paths":["public/new.txt"]
                })
            )
            .unwrap(),
            Some(Behavior::Allow)
        );
        assert_eq!(
            policy_for(
                &rules,
                "edit_file",
                &json!({
                    "path":"secrets", "paths":["secrets"]
                })
            )
            .unwrap(),
            Some(Behavior::Deny)
        );
        assert_eq!(
            policy_for(
                &rules,
                "read_file",
                &json!({
                    "path":"secrets/key.txt", "paths":["secrets/key.txt"]
                })
            )
            .unwrap(),
            None
        );
        assert_eq!(
            policy_for(
                &rules,
                "notebook_edit",
                &json!({
                    "path":"secrets/private.ipynb", "paths":["secrets/private.ipynb"]
                })
            )
            .unwrap(),
            Some(Behavior::Deny)
        );
    }

    #[test]
    fn original_shell_rule_permission_suggestions_are_exact() {
        // Ref: src/utils/permissions/shellRuleMatching.ts:189-228.
        assert_eq!(
            suggestion_for_exact_command("Bash", "git status"),
            json!([{
                "type":"addRules",
                "rules":[{"toolName":"Bash","ruleContent":"git status"}],
                "behavior":"allow",
                "destination":"localSettings"
            }])
        );
        assert_eq!(
            suggestion_for_prefix("Bash", "npm"),
            json!([{
                "type":"addRules",
                "rules":[{"toolName":"Bash","ruleContent":"npm:*"}],
                "behavior":"allow",
                "destination":"localSettings"
            }])
        );
        assert_eq!(
            suggestion_for_prefix("PowerShell", "Get-ChildItem"),
            json!([{
                "type":"addRules",
                "rules":[{"toolName":"PowerShell","ruleContent":"Get-ChildItem:*"}],
                "behavior":"allow",
                "destination":"localSettings"
            }])
        );
        assert_eq!(
            suggestion_for_exact_command("Bash", "echo 'a:b(*)'")[0]["rules"][0]["ruleContent"],
            "echo 'a:b(*)'"
        );
    }

    #[test]
    fn original_wildcard_single_trailing_optional_args() {
        assert!(match_wildcard("git *", "git", false).unwrap());
        assert!(match_wildcard("git *", "git status", false).unwrap());
        assert!(!match_wildcard("git *", "gitx", false).unwrap());
        assert!(!match_wildcard("npm * run *", "npm run", false).unwrap());
        assert!(match_wildcard("npm * run *", "npm ci run test", false).unwrap());
        assert!(match_wildcard("GIT *", "git status", true).unwrap());
        assert!(!match_wildcard("GIT *", "git status", false).unwrap());
        assert!(match_wildcard("a*b", "a\nb", false).unwrap());
        assert_eq!(extract_prefix("npm:*"), Some("npm"));
        assert!(!has_wildcards("npm:*"));
    }

    #[test]
    fn read_rules_apply_to_requested_and_resolved_paths_with_glob_semantics() {
        let rules = compile_rules(&[
            RuleSetting {
                behavior: "deny".into(),
                rule: "Read(**/.env)".into(),
            },
            RuleSetting {
                behavior: "ask".into(),
                rule: "Read(secrets/**)".into(),
            },
            RuleSetting {
                behavior: "allow".into(),
                rule: "Read(public/**)".into(),
            },
        ])
        .unwrap();
        assert_eq!(canonical_tool("read_file"), "Read");
        assert_eq!(
            policy_for(
                &rules,
                "read_file",
                &json!({
                    "path":"alias","paths":["alias","/workspace/.env",".env"]
                })
            )
            .unwrap(),
            Some(Behavior::Deny)
        );
        assert_eq!(
            policy_for(
                &rules,
                "read_file",
                &json!({
                    "path":"secrets/nested/key.txt","paths":["secrets/nested/key.txt"]
                })
            )
            .unwrap(),
            Some(Behavior::Ask)
        );
        assert_eq!(
            policy_for(
                &rules,
                "read_file",
                &json!({
                    "path":"secrets","paths":["secrets"]
                })
            )
            .unwrap(),
            Some(Behavior::Ask)
        );
        assert_eq!(
            policy_for(
                &rules,
                "read_file",
                &json!({
                    "path":"public/hello.txt","paths":["public/hello.txt"]
                })
            )
            .unwrap(),
            Some(Behavior::Allow)
        );
        assert_eq!(
            policy_for(
                &rules,
                "read_file",
                &json!({
                    "path":"ordinary.txt","paths":["ordinary.txt"]
                })
            )
            .unwrap(),
            None
        );
        let all = compile_rules(&[RuleSetting {
            behavior: "deny".into(),
            rule: "Read".into(),
        }])
        .unwrap();
        assert_eq!(
            policy_for(&all, "read_file", &json!({"path":"ordinary.txt"})).unwrap(),
            Some(Behavior::Deny)
        );
    }
    #[test]
    fn webfetch_domain_content_rules_match_exact_url_hostname_only() {
        let rules = compile_rules(&[
            RuleSetting {
                behavior: "allow".into(),
                rule: "WebFetch(domain:api.example.com)".into(),
            },
            RuleSetting {
                behavior: "deny".into(),
                rule: "WebFetch(domain:blocked.example.com)".into(),
            },
            RuleSetting {
                behavior: "ask".into(),
                rule: "WebFetch(domain:api.example.com)".into(),
            },
        ])
        .unwrap();
        assert_eq!(
            policy_for(
                &rules,
                "web_fetch",
                &json!({"url":"https://API.EXAMPLE.COM:8443/path?q=1"})
            )
            .unwrap(),
            Some(Behavior::Ask)
        );
        assert_eq!(
            policy_for(
                &rules,
                "WebFetch",
                &json!({"url":"https://blocked.example.com/"})
            )
            .unwrap(),
            Some(Behavior::Deny)
        );
        assert_eq!(
            policy_for(
                &rules,
                "web_fetch",
                &json!({"url":"https://api.example.com.evil.invalid/"})
            )
            .unwrap(),
            None
        );
        assert_eq!(
            policy_for(&rules, "web_fetch", &json!({"url":"invalid url"})).unwrap(),
            None
        );
        assert_eq!(
            policy_for(
                &rules,
                "web_fetch",
                &json!({"url":"https://other.example.com"})
            )
            .unwrap(),
            None
        );
        let only_allow = compile_rules(&[RuleSetting {
            behavior: "allow".into(),
            rule: "WebFetch(domain:example.org)".into(),
        }])
        .unwrap();
        assert_eq!(
            policy_for(
                &only_allow,
                "web_fetch",
                &json!({"url":"https://example.org/index.html"})
            )
            .unwrap(),
            Some(Behavior::Allow)
        );
        assert_eq!(
            policy_for(
                &only_allow,
                "web_fetch",
                &json!({"url":"https://example.org.evil.invalid/"})
            )
            .unwrap(),
            None
        );
        // Original getRuleByContentsForTool performs an exact key lookup.
        let wildcard = compile_rules(&[RuleSetting {
            behavior: "allow".into(),
            rule: "WebFetch(domain:*)".into(),
        }])
        .unwrap();
        assert_eq!(
            policy_for(
                &wildcard,
                "web_fetch",
                &json!({"url":"https://example.org"})
            )
            .unwrap(),
            None
        );
    }
    #[test]
    fn mcp_server_permissions_match_original_server_and_wildcard_scope() {
        let rules = compile_rules(&[
            RuleSetting {
                behavior: "allow".into(),
                rule: "mcp__github".into(),
            },
            RuleSetting {
                behavior: "deny".into(),
                rule: "mcp__secrets__*".into(),
            },
            RuleSetting {
                behavior: "ask".into(),
                rule: "mcp__github__delete".into(),
            },
        ])
        .unwrap();
        assert_eq!(
            policy_for(&rules, "mcp__github__read", &json!({})).unwrap(),
            Some(Behavior::Allow)
        );
        assert_eq!(
            policy_for(&rules, "mcp__github__delete", &json!({})).unwrap(),
            Some(Behavior::Ask)
        );
        assert_eq!(
            policy_for(&rules, "mcp__secrets__read", &json!({})).unwrap(),
            Some(Behavior::Deny)
        );
        assert_eq!(
            policy_for(&rules, "mcp__other__read", &json!({})).unwrap(),
            None
        );
        assert_eq!(
            policy_for(&rules, "mcp__githubish__read", &json!({})).unwrap(),
            None
        );
        assert_eq!(policy_for(&rules, "Read", &json!({})).unwrap(), None);
        assert!(!mcp_server_rule_matches(
            "mcp__github__all",
            "mcp__github__other"
        ));
        assert!(!mcp_server_rule_matches(
            "mcp__github__",
            "mcp__github__other"
        ));
    }
    #[test]
    fn mcp_server_rule_preserves_deny_over_allow_without_matching_builtin_tools() {
        let rules = compile_rules(&[
            RuleSetting {
                behavior: "allow".into(),
                rule: "mcp__local__*".into(),
            },
            RuleSetting {
                behavior: "deny".into(),
                rule: "mcp__local".into(),
            },
            RuleSetting {
                behavior: "deny".into(),
                rule: "Write".into(),
            },
        ])
        .unwrap();
        assert_eq!(
            policy_for(&rules, "mcp__local__Write", &json!({"path":"file"})).unwrap(),
            Some(Behavior::Deny)
        );
        assert_eq!(
            policy_for(&rules, "Write", &json!({"path":"file"})).unwrap(),
            Some(Behavior::Deny)
        );
        let builtins = compile_rules(&[RuleSetting {
            behavior: "deny".into(),
            rule: "Write".into(),
        }])
        .unwrap();
        assert_eq!(
            policy_for(&builtins, "mcp__local__Write", &json!({})).unwrap(),
            None
        );
        let with_content = compile_rules(&[RuleSetting {
            behavior: "allow".into(),
            rule: "mcp__local(repo:*)".into(),
        }])
        .unwrap();
        assert_eq!(
            policy_for(&with_content, "mcp__local__Write", &json!({})).unwrap(),
            None
        );
    }
    #[test]
    fn persisted_rule_validation_matches_original_settings_filter() {
        for valid in [
            "Bash(git status)",
            "Bash(git:*)",
            "Bash(git *)",
            "Read(src/**)",
            "WebFetch(domain:example.com)",
            "WebSearch(rust language)",
            "mcp__server__tool",
            "mcp__server__*",
            "KillShell(123)",
        ] {
            assert!(valid_stored_rule(valid), "valid: {valid}");
        }
        for invalid in [
            "",
            " ",
            "Bash()",
            "Bash(echo (",
            "Bash(echo ))",
            "Bash(npm:* run)",
            "Bash(:*)",
            "Read(path:*)",
            "Read(path*name)",
            "WebFetch(https://example.com)",
            "WebFetch(example.com)",
            "WebSearch(foo*)",
            "mcp__server(*)",
            "mcp__",
            "read_file(foo)",
        ] {
            assert!(!valid_stored_rule(invalid), "invalid: {invalid}");
        }
    }

    #[test]
    fn parsing_and_escaping_reference_cases() {
        for (text, name, content) in [
            ("Bash", "Bash", None),
            ("Bash(npm install)", "Bash", Some("npm install")),
            (
                "Bash(python -c \"print\\(1\\)\")",
                "Bash",
                Some("python -c \"print(1)\""),
            ),
            ("Bash(*)", "Bash", None),
            ("Bash()", "Bash", None),
            ("(foo)", "(foo)", None),
            ("Bash(a)b", "Bash(a)b", None),
            ("KillShell(abc)", "TaskStop", Some("abc")),
            ("Task", "Agent", None),
        ] {
            let parsed = parse_rule(text);
            assert_eq!(parsed.tool_name, name);
            assert_eq!(parsed.rule_content.as_deref(), content);
        }
        let values = ["abc\\(hi\\)", "x(a)", "hey\\\\cmd", "😀 (hello)"];
        for value in values {
            assert_eq!(unescape_content(&escape_content(value)), value);
        }
        assert_eq!(
            legacy_names("TaskOutput"),
            vec!["AgentOutputTool", "BashOutputTool"]
        );
    }
    #[test]
    fn wildcard_and_legacy_prefix_match_reference_cases() {
        assert!(!has_wildcards("npm:*"));
        assert!(!has_wildcards(r"literal\*"));
        assert!(has_wildcards("git *"));
        for (pattern, command, matched) in [
            ("git *", "git status", true),
            ("git *", "git", true),
            ("git *", "gitstatus", false),
            ("npm:*", "npm", true),
            ("npm:*", "npm run test", true),
            ("npm:*", "npminstall", false),
            (r"echo \*", "echo *", false),
            (r"echo \*", "echo hey", false),
            ("npm * install", "npm local install", true),
            ("npm * install", "npm local\ninstall", false),
            ("npm * install", "npm local\n install", true),
            ("foo", "foobar", false),
            ("foo", "foo", true),
        ] {
            assert_eq!(
                matches_content(pattern, command).unwrap(),
                matched,
                "{pattern} vs {command}"
            );
        }
        assert!(match_wildcard(r"echo \*", "echo *", false).unwrap());
        assert!(match_wildcard("GIT *", "git status", true).unwrap());
    }

    #[test]
    fn compound_shell_permissions_cannot_hide_unapproved_commands() {
        let rules = compile_rules(&[
            RuleSetting {
                behavior: "allow".into(),
                rule: "Bash(printf *)".into(),
            },
            RuleSetting {
                behavior: "allow".into(),
                rule: "Bash(touch allowed)".into(),
            },
            RuleSetting {
                behavior: "deny".into(),
                rule: "Bash(touch forbidden)".into(),
            },
            RuleSetting {
                behavior: "ask".into(),
                rule: "Bash(touch review)".into(),
            },
        ])
        .unwrap();
        let decide =
            |command: &str| policy_for(&rules, "run_command", &json!({"command":command})).unwrap();
        assert_eq!(decide("printf ok && touch allowed"), Some(Behavior::Allow));
        assert_eq!(decide("printf ok; touch forbidden"), Some(Behavior::Deny));
        assert_eq!(decide("printf ok || touch review"), Some(Behavior::Ask));
        assert_eq!(decide("printf ok | touch unknown"), None);
        assert_eq!(decide("printf ok && touch unknown"), None);
        assert_eq!(decide("printf 'a; b'"), Some(Behavior::Allow));
        assert_eq!(decide("printf ok$(touch hidden)"), None);
        assert_eq!(decide("printf ok > unsafe"), None);
        assert_eq!(decide("printf ok & touch allowed"), None);
        assert_eq!(decide("bash -c 'touch unknown'"), None);
        let explicit = compile_rules(&[RuleSetting {
            behavior: "allow".into(),
            rule: "Bash(printf ok > exact)".into(),
        }])
        .unwrap();
        assert_eq!(
            policy_for(
                &explicit,
                "run_command",
                &json!({"command":"printf ok > exact"})
            )
            .unwrap(),
            Some(Behavior::Allow)
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn compound_permission_decisions_reach_real_shell_and_filesystem() {
        use crate::{
            protocol::{FunctionCall, ToolCall},
            tools::{Approval, ToolExecutor},
        };
        let workspace = tempfile::tempdir().unwrap();
        let rules = compile_rules(&[
            RuleSetting {
                behavior: "allow".into(),
                rule: "Bash(touch allowed)".into(),
            },
            RuleSetting {
                behavior: "allow".into(),
                rule: "Bash(printf *)".into(),
            },
            RuleSetting {
                behavior: "deny".into(),
                rule: "Bash(touch forbidden)".into(),
            },
        ])
        .unwrap();
        let executor = ToolExecutor::new(
            workspace.path().canonicalize().unwrap(),
            Approval::Ask,
            false,
        )
        .with_permission_rules(rules);
        let call = |command: &str| ToolCall {
            id: "shell-test".into(),
            kind: "function".into(),
            function: FunctionCall {
                name: "run_command".into(),
                arguments: json!({"command":command}).to_string(),
            },
        };
        let success = executor.execute(&call("touch allowed && printf ok")).await;
        assert!(success.contains("Exit: exit status: 0"), "{success}");
        assert!(workspace.path().join("allowed").exists());
        let denied = executor
            .execute(&call("printf ok && touch forbidden"))
            .await;
        assert!(denied.starts_with("ERROR:"), "{denied}");
        assert!(!workspace.path().join("forbidden").exists());
        let unknown = executor.execute(&call("printf ok && touch unknown")).await;
        assert!(unknown.starts_with("ERROR:"), "{unknown}");
        assert!(!workspace.path().join("unknown").exists());
        let injection = executor.execute(&call("printf ok$(touch hidden)")).await;
        assert!(injection.starts_with("ERROR:"), "{injection}");
        assert!(!workspace.path().join("hidden").exists());
    }

    #[test]
    fn explicit_deny_precedes_allow_and_ask_overrides_allow() {
        let entries = compile_rules(&[
            RuleSetting {
                behavior: "allow".into(),
                rule: "Bash(git *)".into(),
            },
            RuleSetting {
                behavior: "deny".into(),
                rule: "Bash(git push *)".into(),
            },
            RuleSetting {
                behavior: "ask".into(),
                rule: "Bash(git reset *)".into(),
            },
        ])
        .unwrap();
        assert_eq!(
            policy_for(&entries, "run_command", &json!({"command":"git status"})).unwrap(),
            Some(Behavior::Allow)
        );
        assert_eq!(
            policy_for(
                &entries,
                "run_command",
                &json!({"command":"git push origin main"})
            )
            .unwrap(),
            Some(Behavior::Deny)
        );
        assert_eq!(
            policy_for(
                &entries,
                "run_command",
                &json!({"command":"git reset --hard"})
            )
            .unwrap(),
            Some(Behavior::Ask)
        );
        assert_eq!(
            policy_for(&entries, "run_command", &json!({"command":"ls"})).unwrap(),
            None
        );
        assert!(compile_rules(&[RuleSetting {
            behavior: "approve".into(),
            rule: "Bash".into()
        }])
        .is_err());
    }
}
