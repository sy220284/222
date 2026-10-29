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
            "max_results":{"type":"integer","minimum":1,"maximum":100,"default":5}
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
        if name.starts_with("mcp__") {
            deferred_names.push(name.to_owned());
            if !loaded.contains(name) {
                continue;
            }
        }
        visible.push(tool.clone());
    }
    (visible, deferred_names)
}

pub fn execute(args: &Value, tools: &[Value]) -> Result<Value> {
    let query = args
        .get("query")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("ToolSearch query must be a string"))?
        .trim();
    if query.is_empty() {
        bail!("ToolSearch query cannot be empty");
    }
    let max = match args.get("max_results") {
        None => 5,
        Some(v) => match v.as_u64() {
            Some(n @ 1..=100) => n as usize,
            _ => bail!("max_results must be an integer between 1 and 100"),
        },
    };
    let catalog: Vec<(&str, &str)> = tools
        .iter()
        .filter_map(|item| {
            let function = item.get("function")?;
            Some((
                function.get("name")?.as_str()?,
                function
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or(""),
            ))
        })
        .filter(|(name, _)| *name != "ToolSearch")
        .collect();
    // Current provider-neutral protocol exposes active tools, including MCP,
    // directly. Report deferred MCP count without misrepresenting unavailable tools.
    let deferred_count = catalog
        .iter()
        .filter(|(name, _)| name.starts_with("mcp__"))
        .count();
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
            if let Some((name, _)) = catalog
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(requested))
            {
                if !found.iter().any(|existing: &String| existing == name) {
                    found.push((*name).to_owned());
                }
            }
        }
        matches = found;
    } else if let Some((name, _)) = catalog
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(query))
    {
        matches = vec![(*name).to_owned()];
    } else if query.starts_with("mcp__")
        && catalog.iter().any(|(name, _)| {
            name.to_ascii_lowercase()
                .starts_with(&query.to_ascii_lowercase())
        })
    {
        matches = catalog
            .iter()
            .filter(|(name, _)| {
                name.to_ascii_lowercase()
                    .starts_with(&query.to_ascii_lowercase())
            })
            .take(max)
            .map(|(name, _)| (*name).to_owned())
            .collect();
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
        for (name, description) in &catalog {
            let components = parts(name);
            let desc = description.to_ascii_lowercase();
            let mcp = name.starts_with("mcp__");
            let mut score = 0i32;
            let mut required_match = true;
            for (term, pattern) in all_terms.iter().zip(&patterns) {
                let exact = components.iter().any(|p| p == term);
                let partial = components.iter().any(|p| p.contains(term));
                let in_description = pattern.is_match(&desc);
                if required.contains(term) && !partial && !in_description {
                    required_match = false;
                    break;
                }
                if exact {
                    score += if mcp { 12 } else { 10 };
                } else if partial {
                    score += if mcp { 6 } else { 5 };
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
        matches = scored.into_iter().take(max).map(|(name, _)| name).collect();
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
    Ok(json!({"matches":matches,"query":query,
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
    fn invalid_query_is_rejected() {
        assert!(execute(&json!({"query":""}), &catalog()).is_err());
        assert!(execute(&json!({"query":"x","max_results":0}), &catalog()).is_err());
        assert!(execute(&json!({"query":"x","max_results":1.5}), &catalog()).is_err());
    }
}
