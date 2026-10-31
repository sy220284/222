//! Discover active model tools. Ported from 2.1.88 ToolSearchTool name
//! parsing, explicit selection, required keywords and weighted ranking.
use anyhow::{bail, Result};
use regex::Regex;
use serde_json::{json, Value};

pub fn definition() -> Value {
    json!({"type":"function","function":{
        "name":"ToolSearch",
        "description":"Find tools by name, server, capability or description. Use select:tool_name[,tool_name] to select exact tools, or +required terms.",
        "parameters":{"type":"object","properties":{
            "query":{"type":"string","description":"Tool name, select: names, or search keywords"},
            "max_results":{"type":"number","default":5}
        },"required":["query"]}
    }})
}

fn parts(name: &str) -> Vec<String> {
    if let Some(rest) = name.strip_prefix("mcp__") {
        return rest
            .split("__")
            .flat_map(|x| x.split('_'))
            .filter(|s| !s.is_empty())
            .map(str::to_ascii_lowercase)
            .collect();
    }
    let mut words = Vec::new();
    let mut current = String::new();
    let mut previous_lower = false;
    for c in name.chars() {
        if c == '_' || c == '-' || c.is_whitespace() {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
            previous_lower = false;
            continue;
        }
        if previous_lower && c.is_uppercase() && !current.is_empty() {
            words.push(std::mem::take(&mut current));
        }
        current.extend(c.to_lowercase());
        previous_lower = c.is_lowercase() || c.is_ascii_digit();
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

/// Original ToolSearchTool/prompt.ts::isDeferredTool: selected tools are
/// discoverable by name until the model explicitly requests their schemas.
/// MCP tools are deferred by default; the original also defers interactive,
/// task, web and editor tools. Tool metadata can override this per tool.
fn is_deferred(tool: &Value) -> bool {
    let function = &tool["function"];
    let Some(name) = function["name"].as_str() else {
        return false;
    };
    if tool["alwaysLoad"] == true
        || function["alwaysLoad"] == true
        || tool.pointer("/_meta/alwaysLoad") == Some(&Value::Bool(true))
    {
        return false;
    }
    if name == "ToolSearch" {
        return false;
    }
    if tool["shouldDefer"] == true || function["shouldDefer"] == true {
        return true;
    }
    name.starts_with("mcp__")
        || matches!(
            name,
            "AskUserQuestion"
                | "web_search"
                | "web_fetch"
                | "notebook_edit"
                | "task_output"
                | "task_stop"
                | "TodoWrite"
                | "TaskCreate"
                | "TaskGet"
                | "TaskList"
                | "TaskUpdate"
                | "task_create"
                | "task_list"
                | "task_update"
                | "TeamCreate"
                | "TeamDelete"
                | "SendMessage"
                | "lsp_query"
        )
}

/// MCP definitions are discoverable by name but not sent in full on turn one.
/// Selected schemas become visible on the next model request.
pub fn visible_catalog(
    all: &[Value],
    loaded: &std::collections::HashSet<String>,
) -> (Vec<Value>, Vec<String>) {
    let mut visible = Vec::new();
    let mut deferred_names = Vec::new();
    for tool in all {
        let name = tool["function"]["name"].as_str().unwrap_or("");
        if is_deferred(tool) {
            deferred_names.push(name.to_owned());
            if !loaded.contains(name) {
                continue;
            }
        }
        // Internal discovery hints are not valid fields in OpenAI-compatible
        // function schemas. Keep them in the searchable catalog only.
        let mut schema = tool.clone();
        if let Some(fields) = schema.as_object_mut() {
            for key in ["alwaysLoad", "searchHint", "shouldDefer", "_meta"] {
                fields.remove(key);
            }
        }
        visible.push(schema);
    }
    (visible, deferred_names)
}

/// JavaScript Array.slice(0, maxResults): truncates fractional numbers,
/// and a negative end omits that many items from the tail.
fn js_slice_end(len: usize, max_results: f64) -> usize {
    let end = max_results.trunc();
    if end >= 0.0 {
        (end as usize).min(len)
    } else {
        len.saturating_sub((-end) as usize)
    }
}

pub fn execute(args: &Value, tools: &[Value]) -> Result<Value> {
    let original_query = args
        .get("query")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("ToolSearch query must be a string"))?;
    let query = original_query.trim();
    let max = match args.get("max_results") {
        None => 5.0,
        Some(value) => value
            .as_f64()
            .ok_or_else(|| anyhow::anyhow!("max_results must be a number"))?,
    };
    let catalog: Vec<(&str, &str, &str)> = tools
        .iter()
        .filter_map(|item| {
            let function = item.get("function")?;
            Some((
                function.get("name")?.as_str()?,
                function
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or(""),
                item.get("searchHint")
                    .or_else(|| function.get("searchHint"))
                    .and_then(Value::as_str)
                    .unwrap_or(""),
            ))
        })
        .filter(|(name, _, _)| *name != "ToolSearch")
        .collect();
    // Count and rank only tools that actually require schema discovery.
    // Exact name selection may still return an already-loaded tool.
    let deferred_names: std::collections::HashSet<&str> = tools
        .iter()
        .filter(|tool| is_deferred(tool))
        .filter_map(|tool| tool.pointer("/function/name").and_then(Value::as_str))
        .collect();
    let deferred_count = deferred_names.len();
    let matches: Vec<String>;
    if query
        .get(..7)
        .is_some_and(|s| s.eq_ignore_ascii_case("select:"))
    {
        let mut found = Vec::new();
        for requested in query[7..]
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            if let Some((name, _, _)) = catalog
                .iter()
                .find(|(name, _, _)| name.eq_ignore_ascii_case(requested))
            {
                if !found.iter().any(|existing: &String| existing == name) {
                    found.push((*name).to_owned());
                }
            }
        }
        matches = found;
    } else if let Some((name, _, _)) = catalog
        .iter()
        .find(|(name, _, _)| name.eq_ignore_ascii_case(query))
    {
        matches = vec![(*name).to_owned()];
    } else if query.to_ascii_lowercase().starts_with("mcp__")
        && catalog.iter().any(|(name, _, _)| {
            name.to_ascii_lowercase()
                .starts_with(&query.to_ascii_lowercase())
        })
    {
        matches = catalog
            .iter()
            .filter(|(name, _, _)| {
                deferred_names.contains(name)
                    && name
                        .to_ascii_lowercase()
                        .starts_with(&query.to_ascii_lowercase())
            })
            .map(|(name, _, _)| (*name).to_owned())
            .collect::<Vec<_>>();
        matches.truncate(js_slice_end(matches.len(), max));
    } else {
        let words: Vec<String> = query
            .split_whitespace()
            .map(str::to_ascii_lowercase)
            .collect();
        let required: Vec<&str> = words
            .iter()
            .filter_map(|w| w.strip_prefix('+'))
            .filter(|w| !w.is_empty())
            .collect();
        let all_terms: Vec<&str> = words
            .iter()
            .map(|w| w.strip_prefix('+').unwrap_or(w))
            .collect();
        let patterns: Vec<Regex> = all_terms
            .iter()
            .map(|w| Regex::new(&format!(r"\b{}\b", regex::escape(w))))
            .collect::<std::result::Result<_, _>>()?;
        let mut scored = Vec::new();
        for (name, description, hint) in &catalog {
            if !deferred_names.contains(name) {
                continue;
            }
            let components = parts(name);
            let full_name = components.join(" ");
            let desc = description.to_ascii_lowercase();
            let hint = hint.to_ascii_lowercase();
            let mcp = name.starts_with("mcp__");
            let mut score = 0i32;
            let mut required_match = true;
            for (term, pattern) in all_terms.iter().zip(&patterns) {
                let exact = components.iter().any(|p| p == term);
                let partial = components.iter().any(|p| p.contains(term));
                let in_description = pattern.is_match(&desc);
                let in_hint = pattern.is_match(&hint);
                if required.contains(term) && !partial && !in_description && !in_hint {
                    required_match = false;
                    break;
                }
                if exact {
                    score += if mcp { 12 } else { 10 };
                } else if partial {
                    score += if mcp { 6 } else { 5 };
                }
                // Source ToolSearchTool.ts:277-290: full-name fallback and
                // curated search hints rank ahead of generic descriptions.
                if full_name.contains(term) && score == 0 {
                    score += 3;
                }
                if in_hint {
                    score += 4;
                }
                if in_description {
                    score += 2;
                }
            }
            if required_match && score > 0 {
                scored.push(((*name).to_owned(), score));
            }
        }
        scored.sort_by(|a, b| b.1.cmp(&a.1));
        matches = scored.into_iter().map(|(name, _)| name).collect();
        matches.truncate(js_slice_end(matches.len(), max));
    }
    let schemas: Vec<Value> = tools
        .iter()
        .filter(|tool| {
            tool["function"]["name"]
                .as_str()
                .is_some_and(|name| matches.iter().any(|selected| selected == name))
        })
        .cloned()
        .collect();
    Ok(json!({"matches":matches,"query":original_query,
        "total_deferred_tools":deferred_count,"schemas":schemas}))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn catalog() -> Vec<Value> {
        vec![
            json!({"function":{"name":"mcp__github__search_issues","description":"Search repository issues"}}),
            json!({"function":{"name":"mcp__github__create_issue","description":"Create issues"}}),
            json!({"function":{"name":"ReadFile","description":"Read source files"}}),
            definition(),
        ]
    }
    #[test]
    fn exact_selection_dedup_and_missing() {
        assert_eq!(
            execute(
                &json!({"query":"select:ReadFile,missing,readfile"}),
                &catalog()
            )
            .unwrap()["matches"],
            json!(["ReadFile"])
        );
        assert_eq!(
            execute(&json!({"query":"mcp__github"}), &catalog()).unwrap()["matches"],
            json!(["mcp__github__search_issues", "mcp__github__create_issue"])
        );
    }
    #[test]
    fn keyword_and_required_terms() {
        let found = execute(&json!({"query":"+github search"}), &catalog()).unwrap();
        assert_eq!(
            found["matches"],
            json!(["mcp__github__search_issues", "mcp__github__create_issue"])
        );
        assert_eq!(found["total_deferred_tools"], 2);
        assert_eq!(
            execute(&json!({"query":"issues","max_results":1}), &catalog()).unwrap()["matches"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }
    #[test]
    fn dynamic_mcp_catalog_requires_explicit_selection() {
        let all = catalog();
        let mut loaded = std::collections::HashSet::new();
        let (before, deferred) = visible_catalog(&all, &loaded);
        assert_eq!(deferred.len(), 2);
        assert!(!before
            .iter()
            .any(|tool| { tool["function"]["name"] == "mcp__github__search_issues" }));
        let search = execute(&json!({"query":"select:mcp__github__search_issues"}), &all).unwrap();
        assert_eq!(search["schemas"].as_array().unwrap().len(), 1);
        for value in search["matches"].as_array().unwrap() {
            loaded.insert(value.as_str().unwrap().to_owned());
        }
        let (after, _) = visible_catalog(&all, &loaded);
        assert!(after
            .iter()
            .any(|tool| { tool["function"]["name"] == "mcp__github__search_issues" }));
        assert!(!after
            .iter()
            .any(|tool| { tool["function"]["name"] == "mcp__github__create_issue" }));
    }
    #[test]
    fn neutral_metadata_controls_deferred_schema() {
        let deferred = json!({"function":{"name":"mcp__example__tool"}});
        assert_eq!(visible_catalog(&[deferred], &Default::default()).0.len(), 0);
        let always = json!({"_meta":{"alwaysLoad":true},"function":{"name":"mcp__example__tool"}});
        assert_eq!(visible_catalog(&[always], &Default::default()).0.len(), 1);
    }
    #[test]
    fn numeric_max_results_follows_source_javascript_slice_semantics() {
        let two = json!(["mcp__github__search_issues", "mcp__github__create_issue"]);
        assert_eq!(
            execute(&json!({"query":"+github","max_results":2.5}), &catalog()).unwrap()["matches"],
            two
        );
        assert_eq!(
            execute(&json!({"query":"+github","max_results":1.5}), &catalog()).unwrap()["matches"],
            json!(["mcp__github__search_issues"])
        );
        assert_eq!(
            execute(&json!({"query":"+github","max_results":-1}), &catalog()).unwrap()["matches"],
            json!(["mcp__github__search_issues"])
        );
        assert_eq!(
            execute(&json!({"query":"+github","max_results":0}), &catalog()).unwrap()["matches"],
            json!([])
        );
        assert_eq!(
            execute(&json!({"query":" ","max_results":3}), &catalog()).unwrap()["matches"],
            json!([])
        );
        assert_eq!(
            execute(&json!({"query":" x "}), &catalog()).unwrap()["query"],
            " x "
        );
        assert!(execute(&json!({"query":"x","max_results":"1"}), &catalog()).is_err());
    }
}
