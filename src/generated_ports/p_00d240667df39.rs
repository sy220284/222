//! Teammate transcript header translated from src/components/TeammateViewHeader.tsx.
//! The React selector is represented by the already-resolved viewedTeammate
//! snapshot supplied by the host; no global view state is introduced here.
use crate::generated_ports::ui::VNode;
use serde_json::{json, Value};

fn element(tag: &str, attrs: Value, children: Vec<VNode>) -> VNode {
    let attrs = attrs
        .as_object()
        .expect("static view attributes")
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    VNode::Element {
        tag: tag.into(),
        attrs,
        children,
    }
}

pub fn render_source_view(props: &Value) -> VNode {
    let Some(viewed) = props.get("viewedTeammate").filter(|v| !v.is_null()) else {
        return VNode::Empty;
    };
    let Some(name) = viewed
        .pointer("/identity/agentName")
        .and_then(Value::as_str)
    else {
        return VNode::Empty;
    };
    let color = viewed
        .pointer("/identity/color")
        .and_then(Value::as_str)
        .unwrap_or("text");
    let prompt = viewed.get("prompt").and_then(Value::as_str).unwrap_or("");
    element(
        "OffscreenFreeze",
        json!({}),
        vec![element(
            "Box",
            json!({"flexDirection":"column","marginBottom":1}),
            vec![
                element(
                    "Box",
                    json!({}),
                    vec![
                        element("Text", json!({}), vec![VNode::Text("Viewing ".into())]),
                        element(
                            "Text",
                            json!({"color":color,"bold":true}),
                            vec![VNode::Text(format!("@{name}"))],
                        ),
                        element(
                            "Text",
                            json!({"dimColor":true}),
                            vec![
                                VNode::Text(" · ".into()),
                                element(
                                    "KeyboardShortcutHint",
                                    json!({"shortcut":"esc","action":"return"}),
                                    vec![],
                                ),
                            ],
                        ),
                    ],
                ),
                element(
                    "Text",
                    json!({"dimColor":true}),
                    vec![VNode::Text(prompt.into())],
                ),
            ],
        )],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn absent_teammate_is_hidden_like_original_selector() {
        assert_eq!(render_source_view(&json!({})), VNode::Empty);
        assert_eq!(
            render_source_view(&json!({"viewedTeammate":null})),
            VNode::Empty
        );
    }
    #[test]
    fn live_teammate_snapshot_renders_name_task_and_exit_hint() {
        let node = render_source_view(&json!({"viewedTeammate": {
            "identity":{"agentName":"backend","color":"green"},
            "prompt":"Investigate failing tests"
        }}));
        assert_eq!(node.unresolved_node_count(), 0);
        assert!(node.text_content().contains("Viewing @backend"));
        assert!(node.text_content().contains("Investigate failing tests"));
        let VNode::Element { tag, children, .. } = node else {
            panic!("expected freeze wrapper")
        };
        assert_eq!(tag, "OffscreenFreeze");
        let VNode::Element { children, .. } = &children[0] else {
            panic!("expected layout")
        };
        let VNode::Element { children, .. } = &children[0] else {
            panic!("expected header row")
        };
        let VNode::Element { attrs, .. } = &children[1] else {
            panic!("expected teammate name")
        };
        assert_eq!(attrs.get("color"), Some(&json!("green")));
        assert_eq!(attrs.get("bold"), Some(&json!(true)));
    }
}
