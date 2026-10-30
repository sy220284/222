//! Manual port of `src/components/PromptInput/PromptInputFooterSuggestions.tsx`.
//! Terminal dimensions and current selection are supplied by the caller.
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use crate::generated_ports::terminal_truncate::{display_width, truncate_path_middle, truncate_to_width};
use crate::generated_ports::ui::VNode;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SuggestionType {
    #[serde(rename = "command")]
    command,
    #[serde(rename = "file")]
    file,
    #[serde(rename = "directory")]
    directory,
    #[serde(rename = "agent")]
    agent,
    #[serde(rename = "shell")]
    shell,
    #[serde(rename = "custom-title")]
    custom_title,
    #[serde(rename = "slack-channel")]
    slack_channel,
    #[serde(rename = "none")]
    none,
}

pub fn OVERLAY_MAX_ITEMS() -> Value { json!(5) }

fn node(tag: &str, attrs: BTreeMap<String, Value>, children: Vec<VNode>) -> VNode {
    VNode::Element { tag: tag.into(), attrs, children }
}
fn text(content: String, color: Option<&str>, dim: bool, truncate: bool) -> VNode {
    let mut attrs = BTreeMap::from([("dimColor".into(), json!(dim))]);
    if let Some(color) = color { attrs.insert("color".into(), json!(color)); }
    if truncate { attrs.insert("wrap".into(), json!("truncate")); }
    node("Text", attrs, vec![VNode::Text(content)])
}
fn normalize_whitespace(s: &str) -> String {
    // The source uses /\s+/g, including leading and trailing whitespace.
    let mut last_whitespace = false;
    let mut result = String::new();
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !last_whitespace { result.push(' '); }
            last_whitespace = true;
        } else {
            result.push(ch);
            last_whitespace = false;
        }
    }
    result
}
fn columns(props: &Value) -> usize {
    props.get("terminalColumns").and_then(Value::as_u64).unwrap_or(80) as usize
}
fn item_row(item: &Value, width: usize, max_column_width: usize, selected: bool) -> VNode {
    let id = item.get("id").and_then(Value::as_str).expect("suggestion id is required");
    let name = item.get("displayText").and_then(Value::as_str).expect("suggestion displayText is required");
    let description = item.get("description").and_then(Value::as_str).filter(|s| !s.is_empty());
    let selected_color = selected.then_some("suggestion");
    let unified = id.starts_with("file-") || id.starts_with("mcp-resource-") || id.starts_with("agent-");
    if unified {
        let icon = if id.starts_with("mcp-resource-") { "◇" } else if id.starts_with("agent-") { "*" } else { "+" };
        let sep_width: usize = if description.is_some() { 3 } else { 0 };
        let display = if id.starts_with("file-") {
            let reserve = description.map_or(0, |d| display_width(d).min(20));
            let available = width as isize - 2 - 4 - sep_width as isize - reserve as isize;
            truncate_path_middle(name, available)
        } else if id.starts_with("mcp-resource-") {
            truncate_to_width(name, 30)
        } else { name.into() };
        let line = if let Some(description) = description {
            let available = width.saturating_sub(2 + display_width(&display) + sep_width + 4);
            let description = truncate_to_width(&normalize_whitespace(description), available as isize);
            format!("{icon} {display} – {description}")
        } else { format!("{icon} {display}") };
        return text(line, selected_color, !selected, true);
    }

    let display_width_limit = max_column_width.min(width.saturating_mul(4) / 10);
    let name = if display_width(name) > display_width_limit.saturating_sub(2) {
        truncate_to_width(name, display_width_limit as isize - 2)
    } else { name.into() };
    let pad = display_width_limit.saturating_sub(display_width(&name));
    let tag = item.get("tag").and_then(Value::as_str).filter(|s| !s.is_empty())
        .map_or_else(String::new, |tag| format!("[{tag}] "));
    let desc_width = width.saturating_sub(display_width_limit + display_width(&tag) + 4);
    let desc = description
        .map(|d| truncate_to_width(&normalize_whitespace(d), desc_width as isize))
        .unwrap_or_default();
    let explicit_color = item.get("color").and_then(Value::as_str).filter(|s| !s.is_empty());
    let mut children = vec![text(format!("{name}{}", " ".repeat(pad)), explicit_color.or(selected_color), !selected, false)];
    if !tag.is_empty() { children.push(text(tag, None, true, false)); }
    children.push(text(desc, selected_color, !selected, false));
    node("Text", BTreeMap::from([("wrap".into(), json!("truncate"))]), children)
}

pub fn render_source_view(props: &Value) -> VNode {
    let suggestions = props.get("suggestions").and_then(Value::as_array)
        .expect("suggestions must be an array");
    if suggestions.is_empty() { return VNode::Empty; }
    let selected = props.get("selectedSuggestion").and_then(Value::as_i64)
        .expect("selectedSuggestion must be an integer");
    let overlay = props.get("overlay").and_then(Value::as_bool).unwrap_or(false);
    let terminal_rows = props.get("terminalRows").and_then(Value::as_u64).unwrap_or(24) as usize;
    let visible_count = if overlay { 5 } else { terminal_rows.saturating_sub(3).max(1).min(6) };
    let max_column_width = props.get("maxColumnWidth").and_then(Value::as_u64)
        .map(|width| width as usize)
        .unwrap_or_else(|| suggestions.iter()
            .filter_map(|v| v.get("displayText").and_then(Value::as_str))
            .map(display_width).max().unwrap_or(0) + 5);
    let start = (selected - (visible_count / 2) as i64)
        .min(suggestions.len() as i64 - visible_count as i64)
        .max(0) as usize;
    let end = (start + visible_count).min(suggestions.len());
    let selected_id = usize::try_from(selected).ok().and_then(|index| suggestions.get(index))
        .and_then(|item| item.get("id"));
    let rendered = suggestions[start..end].iter().map(|item| {
        let is_selected = selected_id.is_some_and(|id| item.get("id") == Some(id));
        item_row(item, columns(props), max_column_width, is_selected)
    }).collect();
    let mut attrs = BTreeMap::from([("flexDirection".into(), json!("column"))]);
    if !overlay { attrs.insert("justifyContent".into(), json!("flex-end")); }
    node("Box", attrs, rendered)
}

#[cfg(test)]
mod parity_tests {
    use super::*;
    #[test]
    fn empty_suggestions_and_overlay_limit() {
        assert_eq!(render_source_view(&json!({"suggestions": [], "selectedSuggestion":0})), VNode::Empty);
        let suggestions: Vec<_> = (0..12).map(|i| json!({"id":format!("command-{i}"),"displayText":format!("Cmd{i}")})).collect();
        let view = render_source_view(&json!({"suggestions":suggestions,"selectedSuggestion":7,"overlay":true}));
        let VNode::Element {attrs, children, ..} = view else { panic!("box"); };
        assert_eq!(children.len(), 5);
        assert!(!attrs.contains_key("justifyContent"));
        assert!(children[2].text_content().contains("Cmd7"));
    }
    #[test]
    fn narrow_terminal_and_duplicate_ids_follow_source_selection() {
        let suggestions = json!([
            {"id":"file-1","displayText":"src/components/VeryLongFilename.tsx","description":"long description here"},
            {"id":"agent-2","displayText":"Indexer"},
            {"id":"agent-2","displayText":"Indexer duplicate"}
        ]);
        let view = render_source_view(&json!({"suggestions":suggestions,"selectedSuggestion":1,
             "terminalColumns":28,"terminalRows":6}));
        let VNode::Element {attrs, children, ..} = view else {panic!("box")};
        assert_eq!(attrs["justifyContent"],"flex-end");
        assert_eq!(children.len(),3);
        assert_eq!(children[1].text_content(),"* Indexer");
        assert_eq!(children[2].text_content(),"* Indexer duplicate");
        for item in children.into_iter().skip(1) {
            let VNode::Element {attrs, ..} = item else {panic!("row")};
            assert_eq!(attrs["color"],"suggestion");
        }
    }
    #[test]
    fn normal_command_keeps_tag_description_and_explicit_color() {
        let view = render_source_view(&json!({"suggestions":[{"id":"cmd-1","displayText":"/diagnostics",
            "color":"warning", "tag":"local", "description":"some   details"}],
            "selectedSuggestion":0, "terminalColumns":80}));
        assert!(view.text_content().contains("[local]"));
        assert!(view.text_content().contains("some details"));
        let VNode::Element {children,..} = view else {panic!("box")};
        let VNode::Element {children, ..} = &children[0] else {panic!("row")};
        let VNode::Element {attrs, ..} = &children[0] else {panic!("name")};
        assert_eq!(attrs["color"],"warning");
    }
}
