//! Source-aligned teammate status row from src/components/Spinner/TeammateSpinnerLine.tsx.
//! The terminal host supplies width and elapsed-time snapshots from the live session.
use crate::generated_ports::ui::VNode;
use serde_json::{json, Value};

fn element(tag: &str, attrs: Value, children: Vec<VNode>) -> VNode {
    VNode::Element {
        tag: tag.into(),
        attrs: attrs
            .as_object()
            .expect("terminal attributes")
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
        children,
    }
}

fn text(value: impl Into<String>, attrs: Value) -> VNode {
    element("Text", attrs, vec![VNode::Text(value.into())])
}

fn duration(ms: u64) -> String {
    if ms < 60_000 {
        return format!("{}s", ms / 1000);
    }
    let mut secs = (ms % 60_000 + 500) / 1000;
    let mut mins = (ms / 60_000) % 60;
    let mut hours = (ms / 3_600_000) % 24;
    let mut days = ms / 86_400_000;
    if secs == 60 {
        secs = 0;
        mins += 1;
    }
    if mins == 60 {
        mins = 0;
        hours += 1;
    }
    if hours == 24 {
        hours = 0;
        days += 1;
    }
    let mut parts = Vec::new();
    if days > 0 {
        parts.push(format!("{days}d"));
    }
    if hours > 0 {
        parts.push(format!("{hours}h"));
    }
    if mins > 0 {
        parts.push(format!("{mins}m"));
    }
    if secs > 0 {
        parts.push(format!("{secs}s"));
    }
    parts.join(" ")
}

fn formatted_tokens(value: u64) -> String {
    if value < 1000 {
        return value.to_string();
    }
    let (scaled, unit) = if value >= 1_000_000_000 {
        (value as f64 / 1e9, "b")
    } else if value >= 1_000_000 {
        (value as f64 / 1e6, "m")
    } else {
        (value as f64 / 1e3, "k")
    };
    format!("{scaled:.1}{unit}")
}

fn columns_used(value: &str) -> usize {
    value
        .chars()
        .map(|character| {
            let scalar = character as u32;
            if (0x1100..=0x115f).contains(&scalar)
                || (0x2329..=0x232a).contains(&scalar)
                || (0x2e80..=0xa4cf).contains(&scalar)
                || (0xac00..=0xd7a3).contains(&scalar)
                || (0xf900..=0xfaff).contains(&scalar)
                || (0xfe10..=0xfe6f).contains(&scalar)
                || (0xff00..=0xff60).contains(&scalar)
                || scalar >= 0x1f300
            {
                2
            } else {
                1
            }
        })
        .sum()
}

fn truncate(value: &str, max: usize) -> String {
    if columns_used(value) <= max {
        return value.to_owned();
    }
    if max == 0 {
        return String::new();
    }
    let mut result = String::new();
    let mut used = 0;
    for character in value.chars() {
        let current = columns_used(&character.to_string());
        if used + current + 1 > max {
            break;
        }
        used += current;
        result.push(character);
    }
    result.push('…');
    result
}

/// Read recent tool uses and text content in the original reverse-message order.
fn message_preview(messages: &Value) -> Vec<String> {
    let Some(messages) = messages.as_array() else {
        return Vec::new();
    };
    let mut lines = Vec::new();
    for message in messages.iter().rev() {
        if lines.len() >= 3 {
            break;
        }
        if message["type"] != "user" && message["type"] != "assistant" {
            continue;
        }
        let Some(blocks) = message.pointer("/message/content").and_then(Value::as_array) else {
            continue;
        };
        for block in blocks {
            if lines.len() >= 3 {
                break;
            }
            match block["type"].as_str() {
                Some("tool_use") => {
                    let input = &block["input"];
                    let desc = ["description", "prompt", "command", "query", "pattern"]
                        .into_iter()
                        .find_map(|key| {
                            input.get(key).and_then(Value::as_str).filter(|v| !v.is_empty())
                        });
                    let fallback = format!("Using {}…", block["name"].as_str().unwrap_or("tool"));
                    lines.push(truncate(desc.unwrap_or(&fallback).lines().next().unwrap_or(""), 80));
                }
                Some("text") => {
                    if let Some(content) = block["text"].as_str() {
                        for line in content.lines().rev().filter(|line| !line.trim().is_empty()) {
                            if lines.len() >= 3 {
                                break;
                            }
                            lines.push(truncate(line, 80));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    lines.reverse();
    lines
}

fn activity(teammate: &Value) -> &str {
    teammate
        .pointer("/progress/recentActivitySummary")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            teammate
                .pointer("/progress/lastActivity/activityDescription")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
        })
        .or_else(|| teammate["spinnerVerb"].as_str())
        .unwrap_or("working")
}

pub fn render_source_view(props: &Value) -> VNode {
    let teammate = &props["teammate"];
    if !teammate.is_object() {
        return VNode::Empty;
    }
    let last = props["isLast"].as_bool().unwrap_or(false);
    let selected = props["isSelected"].as_bool().unwrap_or(false);
    let foregrounded = props["isForegrounded"].as_bool().unwrap_or(false);
    let highlighted = selected || foregrounded;
    let all_idle = props["allIdle"].as_bool().unwrap_or(false);
    let columns = props["columns"].as_u64().unwrap_or(80) as usize;
    let name = teammate.pointer("/identity/agentName").and_then(Value::as_str).unwrap_or("");
    let color = teammate.pointer("/identity/color").and_then(Value::as_str).unwrap_or("text");
    let tree = match (highlighted, last) {
        (true, true) => "╘═",
        (true, false) => "╞═",
        (false, true) => "└─",
        (false, false) => "├─",
    };
    let tool_count = teammate.pointer("/progress/toolUseCount").and_then(Value::as_u64).unwrap_or(0);
    let tokens = teammate.pointer("/progress/tokenCount").and_then(Value::as_u64).unwrap_or(0);
    let stats = format!(
        " · {tool_count} tool {} · {} tokens",
        if tool_count == 1 { "use" } else { "uses" },
        formatted_tokens(tokens)
    );
    let select_hint = " · shift + ↑/↓ to select";
    let view_hint = " · enter to view";
    let named = format!("@{name}");
    let show_name = columns >= 60
        && columns.saturating_sub(8 + columns_used(&named) + 2) >= 25;
    let name_cost = if show_name { columns_used(&named) + 2 } else { 0 };
    let available = columns.saturating_sub(8 + name_cost);
    let show_view = selected
        && !foregrounded
        && available > columns_used(view_hint) + columns_used(&stats) + 30;
    let show_select = highlighted
        && available
            > columns_used(select_hint)
                + if show_view { columns_used(view_hint) } else { 0 }
                + columns_used(&stats)
                + 30;
    let show_stats = available > columns_used(&stats) + 30;
    let extra = if show_stats { columns_used(&stats) } else { 0 }
        + if show_select { columns_used(select_hint) } else { 0 }
        + if show_view { columns_used(view_hint) } else { 0 };
    let activity_width = 25.max(available.saturating_sub(extra + 1));
    let mut content = vec![
        text(
            if selected { "❯" } else { " " },
            json!({"color":if selected {"suggestion"} else {"text"},"bold":selected}),
        ),
        text(format!("{tree} "), json!({"dimColor":!selected})),
    ];
    if show_name {
        content.push(text(named, json!({"color":if selected {"suggestion"} else {color}})));
        content.push(text(": ", json!({"dimColor":!selected})));
    }
    if teammate["shutdownRequested"] == true {
        content.push(text("[stopping]", json!({"dimColor":true})));
    } else if teammate["awaitingPlanApproval"] == true {
        content.push(text("[awaiting approval]", json!({"color":"warning"})));
    } else if teammate["isIdle"] == true {
        let display = if all_idle {
            let elapsed = props["frozenDuration"].as_str().map(str::to_owned).unwrap_or_else(|| {
                let now = props["nowMs"].as_u64().unwrap_or(0);
                let start = teammate["startTime"].as_u64().unwrap_or(now);
                let paused = teammate["totalPausedMs"].as_u64().unwrap_or(0);
                duration(now.saturating_sub(start).saturating_sub(paused))
            });
            format!(
                "{} for {elapsed}",
                teammate["pastTenseVerb"].as_str().unwrap_or("Worked")
            )
        } else {
            let elapsed = props["idleElapsedTime"].as_str().map(str::to_owned).unwrap_or_else(|| {
                let now = props["nowMs"].as_u64().unwrap_or(0);
                let started = props["idleStartMs"].as_u64().unwrap_or(now);
                duration(now.saturating_sub(started))
            });
            format!("Idle for {elapsed}")
        };
        content.push(text(display, json!({"dimColor":true})));
    } else if !highlighted {
        let value = truncate(activity(teammate), activity_width);
        content.push(text(
            if value.ends_with('…') { value } else { format!("{value}…") },
            json!({"dimColor":true}),
        ));
    }
    if show_stats {
        content.push(text(stats, json!({"dimColor":true})));
    }
    if show_select {
        content.push(text(select_hint, json!({"dimColor":true})));
    }
    if show_view {
        content.push(text(view_hint, json!({"dimColor":true})));
    }
    let mut rows = vec![element("Box", json!({"paddingLeft":3}), content)];
    if props["showPreview"] == true {
        for (index, line) in message_preview(&teammate["messages"]).into_iter().enumerate() {
            rows.push(element(
                "Box",
                json!({"key":index,"paddingLeft":3}),
                vec![
                    text(" ", json!({"dimColor":true})),
                    text(if last { "   " } else { "│  " }, json!({"dimColor":true})),
                    text(line, json!({"dimColor":true})),
                ],
            ));
        }
    }
    element("Box", json!({"flexDirection":"column"}), rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_status_priority_and_tool_count() {
        let node = render_source_view(&json!({
            "teammate":{"identity":{"agentName":"worker","color":"green"},
                "progress":{"toolUseCount":2,"tokenCount":1500,
                    "lastActivity":{"activityDescription":"Scanning files"}}},
            "columns":100
        }));
        assert_eq!(node.unresolved_node_count(), 0);
        assert!(node.text_content().contains("Scanning files… · 2 tool uses · 1.5k tokens"));
        let node = render_source_view(&json!({
            "teammate":{"identity":{"agentName":"worker"},
                "isIdle":true,"shutdownRequested":true,"awaitingPlanApproval":true},
            "columns":90
        }));
        assert!(node.text_content().contains("[stopping]"));
        assert!(!node.text_content().contains("[awaiting approval]"));
    }
    #[test]
    fn respects_narrow_terminal_and_recent_message_preview() {
        let node = render_source_view(&json!({
            "teammate":{"identity":{"agentName":"worker"},"isIdle":true},
            "isLast":true,"allIdle":true,"isSelected":true,"columns":45,"nowMs":60000
        }));
        assert!(node.text_content().contains("╘═ "));
        assert!(!node.text_content().contains("@worker"));
        let node = render_source_view(&json!({
            "teammate":{"identity":{"agentName":"worker"},
                "messages":[{"type":"user","message":{"content":[{"type":"text",
                    "text":"a\nb\nc\nd"}]}}]},
            "showPreview":true,"columns":100
        }));
        assert!(node.text_content().contains("b"));
        assert!(!node.text_content().contains("a\n"));
        assert_eq!(node.unresolved_node_count(), 0);
    }
}
