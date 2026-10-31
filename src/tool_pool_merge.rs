//! Exact pure merge from 2.1.88 hooks/useMergedTools.ts and utils/toolPool.ts.
//! First-seen tool name wins; sorting uses separate built-in and MCP partitions
//! to preserve the provider's built-in-prefix cache boundary.
use anyhow::Result;
use serde_json::Value;
use std::{cmp::Ordering, collections::HashSet};
use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};

pub fn is_pr_activity_subscription_tool(name: &str) -> bool {
    name.ends_with("subscribe_pr_activity") || name.ends_with("unsubscribe_pr_activity")
}

fn tool_name(value: &Value) -> Result<&str> {
    value
        .pointer("/function/name")
        .or_else(|| value.get("name"))
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| anyhow::anyhow!("Tool pool contains a tool without a name"))
}

/// Source tools.ts::getTools: the small tool pool changes when a real
/// coordinator or REPL tool is active. Do not publish tools that are not
/// registered; the caller supplies the already-discovered catalog.
pub fn is_simple_model_tool(name: &str) -> bool {
    is_simple_model_tool_for_context(name, false, false)
}

pub fn is_simple_model_tool_for_context(
    name: &str,
    repl_enabled: bool,
    coordinator_enabled: bool,
) -> bool {
    match crate::tool_names::canonical_name(name) {
        "run_command" | "read_file" | "edit_file" => !repl_enabled,
        "REPL" => repl_enabled,
        "spawn_agent" => coordinator_enabled && !repl_enabled,
        "task_stop" | "SendMessage" => coordinator_enabled,
        _ => false,
    }
}

pub fn filter_simple_model_tools(tools: Vec<Value>) -> Result<Vec<Value>> {
    filter_simple_model_tools_for_context(tools, false, false)
}

pub fn filter_simple_model_tools_for_context(
    tools: Vec<Value>,
    repl_enabled: bool,
    coordinator_enabled: bool,
) -> Result<Vec<Value>> {
    tools
        .into_iter()
        .filter_map(|tool| match tool_name(&tool) {
            Ok(name)
                if is_simple_model_tool_for_context(name, repl_enabled, coordinator_enabled) =>
            {
                Some(Ok(tool))
            }
            Ok(_) => None,
            Err(error) => Some(Err(error)),
        })
        .collect()
}

/// English locale collation for the tool-identifier alphabet. JS localeCompare
/// sorts punctuation and case-insensitive primary weights differently from
/// Rust's byte ordering. Preserve distinct secondary accents and case weights
/// so dynamically discovered MCP names retain stable prompt-cache ordering.
fn collation_key(name: &str) -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    const SYMBOLS: &str = r#" _-,;:!?.'"()[]{}@*/\&#%`^+<=>|~$"#;
    let mut primary = Vec::new();
    let mut accents = Vec::new();
    let mut case = Vec::new();
    for ch in name.nfd() {
        if is_combining_mark(ch) {
            if let Some(last) = accents.last_mut() {
                *last = ch as u32;
            }
            continue;
        }
        let lower = ch.to_lowercase().next().unwrap_or(ch);
        let weight = if let Some(position) = SYMBOLS.find(lower) {
            1 + position as u32
        } else if lower.is_ascii_digit() {
            100 + (lower as u32 - '0' as u32)
        } else if lower.is_ascii_lowercase() {
            200 + (lower as u32 - 'a' as u32)
        } else {
            1000 + lower as u32
        };
        primary.push(weight);
        accents.push(0);
        case.push(u32::from(ch.is_uppercase()));
    }
    (primary, accents, case)
}

fn locale_compare_tool_names(left: &str, right: &str) -> Ordering {
    collation_key(left).cmp(&collation_key(right))
}

/// Initial tools take precedence, matching lodash uniqBy([...initial, ...assembled], 'name').
/// The caller has already applied MCP deny rules during tool discovery.
pub fn merge_and_filter_tools(
    initial: &[Value],
    assembled: &[Value],
    coordinator_allowed: Option<&HashSet<String>>,
) -> Result<Vec<Value>> {
    let mut seen = HashSet::new();
    let mut built_in = Vec::new();
    let mut mcp = Vec::new();
    for tool in initial.iter().chain(assembled) {
        let name = tool_name(tool)?;
        if !seen.insert(name.to_owned()) {
            continue;
        }
        if let Some(allowed) = coordinator_allowed {
            if !allowed.contains(name) && !is_pr_activity_subscription_tool(name) {
                continue;
            }
        }
        // Upstream isMcpTool also recognizes an explicit isMcp marker.
        if name.starts_with("mcp__") || tool.get("isMcp") == Some(&Value::Bool(true)) {
            mcp.push(tool.clone());
        } else {
            built_in.push(tool.clone());
        }
    }
    built_in.sort_by(|a, b| {
        locale_compare_tool_names(
            tool_name(a).expect("validated tool"),
            tool_name(b).expect("validated tool"),
        )
    });
    mcp.sort_by(|a, b| {
        locale_compare_tool_names(
            tool_name(a).expect("validated tool"),
            tool_name(b).expect("validated tool"),
        )
    });
    built_in.extend(mcp);
    Ok(built_in)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tool(name: &str, version: &str) -> Value {
        json!({"type":"function","function":{"name":name,"description":version}})
    }

    #[test]
    fn simple_mode_exposes_only_source_bash_read_and_edit() {
        let catalog = vec![
            tool("read_file", "Read"),
            tool("run_command", "Bash"),
            tool("edit_file", "Edit"),
            tool("write_file", "Write"),
            tool("ToolSearch", "Search"),
            tool("mcp__server__read_file", "MCP"),
        ];
        let simple = filter_simple_model_tools(catalog).unwrap();
        let names = simple
            .iter()
            .map(|tool| tool["function"]["name"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(names, ["read_file", "run_command", "edit_file"]);
        assert!(is_simple_model_tool("Read"));
        assert!(is_simple_model_tool("Bash"));
        assert!(!is_simple_model_tool("Write"));
        assert!(!is_simple_model_tool("mcp__server__read_file"));
        assert!(filter_simple_model_tools(vec![json!({})]).is_err());
    }

    #[test]
    fn simple_mode_coordinator_and_repl_follow_source_branches() {
        let catalog = vec![
            tool("run_command", "Bash"),
            tool("read_file", "Read"),
            tool("edit_file", "Edit"),
            tool("spawn_agent", "Task"),
            tool("TaskStop", "TaskStop"),
            tool("SendMessage", "SendMessage"),
            tool("REPL", "REPL"),
            tool("write_file", "Write"),
        ];
        let coordinator =
            filter_simple_model_tools_for_context(catalog.clone(), false, true).unwrap();
        let names = coordinator
            .iter()
            .map(|tool| tool["function"]["name"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            [
                "run_command",
                "read_file",
                "edit_file",
                "spawn_agent",
                "TaskStop",
                "SendMessage"
            ]
        );
        let repl = filter_simple_model_tools_for_context(catalog, true, true).unwrap();
        let names = repl
            .iter()
            .map(|tool| tool["function"]["name"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(names, ["TaskStop", "SendMessage", "REPL"]);
    }

    #[test]
    fn locale_collation_matches_original_javascript_identifier_fixtures() {
        let names = [
            "Tool-a", "tool_A", "tool-a", "Tool_A", "tools", "tool10", "tool2", "tool", "_tool",
            "-tool", "2tool", "10tool", "abc", "ábc", "Äbc",
        ];
        let mut actual = names.to_vec();
        actual.sort_by(|a, b| locale_compare_tool_names(a, b));
        assert_eq!(
            actual,
            [
                "_tool", "-tool", "10tool", "2tool", "abc", "ábc", "Äbc", "tool", "tool_A",
                "Tool_A", "tool-a", "Tool-a", "tool10", "tool2", "tools",
            ]
        );
        let combined = names
            .iter()
            .map(|name| tool(name, "entry"))
            .collect::<Vec<_>>();
        let merged = merge_and_filter_tools(&[], &combined, None).unwrap();
        assert_eq!(
            merged
                .iter()
                .map(|v| v["function"]["name"].as_str().unwrap())
                .collect::<Vec<_>>(),
            actual
        );
    }

    #[test]
    fn initial_tools_win_and_mcp_never_interleaves_builtin_prefix() {
        let initial = [
            tool("Bash", "initial"),
            tool("mcp__alpha__lookup", "initial"),
        ];
        let assembled = [
            tool("mcp__z__post", "mcp"),
            tool("Read", "built-in"),
            tool("Bash", "assembled"),
            tool("mcp__alpha__lookup", "assembled"),
            tool("Edit", "built-in"),
        ];
        let merged = merge_and_filter_tools(&initial, &assembled, None).unwrap();
        let names: Vec<_> = merged
            .iter()
            .map(|tool| tool.pointer("/function/name").unwrap().as_str().unwrap())
            .collect();
        assert_eq!(
            names,
            ["Bash", "Edit", "Read", "mcp__alpha__lookup", "mcp__z__post"]
        );
        assert_eq!(merged[0]["function"]["description"], "initial");
        assert_eq!(merged[3]["function"]["description"], "initial");
    }

    #[test]
    fn explicit_mcp_marker_is_partitioned_after_builtin_tools() {
        let tools = [
            tool("Read", "builtin"),
            json!({"name":"external_alias","isMcp":true,"description":"MCP"}),
            tool("Write", "builtin"),
        ];
        let merged = merge_and_filter_tools(&[], &tools, None).unwrap();
        assert_eq!(merged[0]["function"]["name"], "Read");
        assert_eq!(merged[1]["function"]["name"], "Write");
        assert_eq!(merged[2]["name"], "external_alias");
    }

    #[test]
    fn coordinator_mode_preserves_subscription_management_tools() {
        let allowed = HashSet::from(["Read".to_owned()]);
        let tools = [
            tool("mcp__git__subscribe_pr_activity", "sub"),
            tool("mcp__git__unsubscribe_pr_activity", "unsub"),
            tool("mcp__git__destructive", "other"),
            tool("Read", "built-in"),
            tool("Bash", "disallowed"),
        ];
        let merged = merge_and_filter_tools(&[], &tools, Some(&allowed)).unwrap();
        assert_eq!(merged.len(), 3);
        assert_eq!(merged[0]["function"]["name"], "Read");
        assert_eq!(
            merged[1]["function"]["name"],
            "mcp__git__subscribe_pr_activity"
        );
        assert_eq!(
            merged[2]["function"]["name"],
            "mcp__git__unsubscribe_pr_activity"
        );
    }

    #[test]
    fn malformed_catalog_is_an_error_not_a_silent_omission() {
        let error = merge_and_filter_tools(&[], &[json!({"type":"function"})], None).unwrap_err();
        assert!(error.to_string().contains("without a name"));
    }
}
