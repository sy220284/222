//! Manual port of `src/components/PromptInput/PromptInputQueuedCommands.tsx`.
//! The caller supplies the current command queue and viewing-agent state.
//! Message normalization/UUID assignment is delegated to the surrounding UI
//! lifecycle to keep UUIDs stable across streaming re-renders.
#![allow(dead_code)]
use crate::generated_ports::ui::VNode;
use serde_json::{json, Value};
use std::collections::BTreeMap;

const MAX_VISIBLE_NOTIFICATIONS: usize = 3;
fn node(tag: &str, attrs: BTreeMap<String, Value>, children: Vec<VNode>) -> VNode {
    VNode::Element {
        tag: tag.into(),
        attrs,
        children,
    }
}
fn is_idle_notification(command: &Value) -> bool {
    command
        .get("value")
        .and_then(Value::as_str)
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        .and_then(|object| object.get("type").cloned())
        .and_then(|value| value.as_str().map(|s| s == "idle_notification"))
        .unwrap_or(false)
}
fn is_visible(command: &Value, channels_enabled: bool) -> bool {
    let channel = command
        .get("origin")
        .and_then(|v| v.get("kind"))
        .and_then(Value::as_str)
        == Some("channel");
    if channels_enabled && channel {
        return true;
    }
    command.get("mode").and_then(Value::as_str) != Some("task-notification")
        && command.get("isMeta").and_then(Value::as_bool) != Some(true)
}
/// The source partitions ordinary commands before task notifications and
/// compresses excess task notifications to two entries and one XML summary.
pub fn process_queued_commands(commands: &[Value]) -> Vec<Value> {
    let mut other = Vec::new();
    let mut notifications = Vec::new();
    for command in commands {
        if is_idle_notification(command) {
            continue;
        }
        if command.get("mode").and_then(Value::as_str) == Some("task-notification") {
            notifications.push(command.clone());
        } else {
            other.push(command.clone());
        }
    }
    if notifications.len() <= MAX_VISIBLE_NOTIFICATIONS {
        other.extend(notifications);
        return other;
    }
    let over = notifications.len() - (MAX_VISIBLE_NOTIFICATIONS - 1);
    other.extend(
        notifications
            .into_iter()
            .take(MAX_VISIBLE_NOTIFICATIONS - 1),
    );
    other.push(json!({"value":format!(
        "<task-notification>\n<summary>+{over} more tasks completed</summary>\n<status>completed</status>\n</task-notification>"
    ),"mode":"task-notification"}));
    other
}
fn content_as_message(command: &Value) -> Value {
    let mode = command.get("mode").and_then(Value::as_str).unwrap_or("");
    let content = command.get("value").cloned().unwrap_or(Value::Null);
    if mode == "bash" {
        if let Some(text) = content.as_str() {
            return json!(format!("<bash-input>{text}</bash-input>"));
        }
    }
    content
}
pub fn render_source_view(props: &Value) -> VNode {
    let commands = props
        .get("queuedCommands")
        .and_then(Value::as_array)
        .expect("queuedCommands must be an array");
    if commands.is_empty()
        || props
            .get("viewingAgentTaskId")
            .is_some_and(crate::generated_ports::runtime::truthy)
    {
        return VNode::Empty;
    }
    let channels_enabled = props
        .get("channelsEnabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let visible: Vec<Value> = commands
        .iter()
        .filter(|command| is_visible(command, channels_enabled))
        .cloned()
        .collect();
    if visible.is_empty() {
        return VNode::Empty;
    }
    let commands = process_queued_commands(&visible);
    if commands.is_empty() {
        return VNode::Empty;
    }
    let is_brief = props.get("briefFeatureEnabled").and_then(Value::as_bool) == Some(true)
        && props.get("isBriefOnly").and_then(Value::as_bool) == Some(true);
    let children = commands
        .iter()
        .enumerate()
        .map(|(i, command)| {
            // The actual source wraps createUserMessage in a memoized normalizeMessages.
            // Preserve the original queued command here so the message owner can
            // assign its stable UUID through the normal message lifecycle.
            let content = content_as_message(command);
            let message = node(
                "Message",
                BTreeMap::from([
                    ("content".into(), content),
                    ("addMargin".into(), json!(false)),
                    ("verbose".into(), json!(false)),
                    ("shouldAnimate".into(), json!(false)),
                    ("shouldShowDot".into(), json!(false)),
                    ("isTranscriptMode".into(), json!(false)),
                    ("isStatic".into(), json!(true)),
                    ("tools".into(), json!([])),
                    ("commands".into(), json!([])),
                    ("inProgressToolUseIDs".into(), json!([])),
                    ("progressMessagesForMessage".into(), json!([])),
                ]),
                vec![],
            );
            node(
                "QueuedMessageProvider",
                BTreeMap::from([
                    ("key".into(), json!(i)),
                    ("isFirst".into(), json!(i == 0)),
                    ("useBriefLayout".into(), json!(is_brief)),
                ]),
                vec![message],
            )
        })
        .collect();
    node(
        "Box",
        BTreeMap::from([
            ("marginTop".into(), json!(1)),
            ("flexDirection".into(), json!("column")),
        ]),
        children,
    )
}

#[cfg(test)]
mod parity_tests {
    use super::*;
    #[test]
    fn idle_notifications_are_hidden_and_task_overflow_is_exact() {
        let commands = vec![
            json!({"value":"hello","mode":"prompt"}),
            json!({"value":"{\"type\":\"idle_notification\"}","mode":"prompt"}),
            json!({"value":"broken {", "mode":"prompt"}),
            json!({"value":"1","mode":"task-notification"}),
            json!({"value":"2","mode":"task-notification"}),
            json!({"value":"3","mode":"task-notification"}),
            json!({"value":"4","mode":"task-notification"}),
        ];
        let results = process_queued_commands(&commands);
        assert_eq!(results.len(), 5);
        assert_eq!(results[0]["value"], "hello");
        assert_eq!(results[1]["value"], "broken {");
        assert!(results[4]["value"]
            .as_str()
            .unwrap()
            .contains("+2 more tasks completed"));
    }
    #[test]
    fn queue_visibility_and_agent_view_gate_match_source() {
        let commands = json!([
            {"value":"hidden","mode":"prompt","isMeta":true},
            {"value":"broadcast","mode":"prompt","isMeta":true,"origin":{"kind":"channel"}},
            {"value":"git status", "mode":"bash"}
        ]);
        let view = render_source_view(&json!({"queuedCommands":commands,"channelsEnabled":true}));
        let VNode::Element { children, .. } = &view else {
            panic!("box")
        };
        assert_eq!(children.len(), 2);
        let VNode::Element { children, .. } = &children[1] else {
            panic!("queued provider")
        };
        let VNode::Element { attrs, .. } = &children[0] else {
            panic!("message")
        };
        assert_eq!(attrs["content"], "<bash-input>git status</bash-input>");
        assert_eq!(
            render_source_view(&json!({"queuedCommands":[],"channelsEnabled":true})),
            VNode::Empty
        );
        assert_eq!(
            render_source_view(
                &json!({"queuedCommands":commands,"viewingAgentTaskId":"subagent-1"})
            ),
            VNode::Empty
        );
    }
}
