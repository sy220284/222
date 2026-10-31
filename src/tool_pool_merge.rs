//! Exact pure merge from 2.1.88 hooks/useMergedTools.ts and utils/toolPool.ts.
//! First-seen tool name wins; sorting uses separate built-in and MCP partitions
//! to preserve the provider's built-in-prefix cache boundary.
use anyhow::Result;
use serde_json::Value;
use std::collections::HashSet;

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
        if name.starts_with("mcp__") {
            mcp.push(tool.clone());
        } else {
            built_in.push(tool.clone());
        }
    }
    // TS localeCompare uses locale-specific collation; ASCII names are
    // stable here (all public built-ins and canonical MCP identifiers).
    built_in.sort_by(|a, b| {
        tool_name(a)
            .expect("validated tool")
            .cmp(tool_name(b).expect("validated tool"))
    });
    mcp.sort_by(|a, b| {
        tool_name(a)
            .expect("validated tool")
            .cmp(tool_name(b).expect("validated tool"))
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
