//! Rust exports derived from original: src/components/messages/GroupedToolUseContent.tsx
//!
//! Render guard equivalence: a tool without renderGroupedToolUse renders
//! nothing. The grouped results map (tool_use_id -> result param/output) is
//! built from the message's result list; the JSX body rides the ink host.

#[derive(Debug, Clone, PartialEq)]
pub struct GroupedResult {
    pub tool_use_id: String,
    pub output: serde_json::Value,
}

/// Build the tool_use_id -> output map from grouped results, preserving
/// first-wins on duplicate ids (Map.set semantics).
pub fn build_results_map(
    results: &[GroupedResult],
) -> std::collections::HashMap<String, serde_json::Value> {
    let mut map = std::collections::HashMap::new();
    for r in results {
        map.entry(r.tool_use_id.clone())
            .or_insert_with(|| r.output.clone());
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn first_wins_on_duplicate_ids() {
        let results = vec![
            GroupedResult {
                tool_use_id: "a".into(),
                output: json!(1),
            },
            GroupedResult {
                tool_use_id: "a".into(),
                output: json!(2),
            },
            GroupedResult {
                tool_use_id: "b".into(),
                output: json!(3),
            },
        ];
        let m = build_results_map(&results);
        assert_eq!(m.get("a"), Some(&json!(1)));
        assert_eq!(m.get("b"), Some(&json!(3)));
    }
}
