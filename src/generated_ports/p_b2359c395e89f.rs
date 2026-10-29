//! Native Rust UI structure derived from original: src/commands/plugin/DiscoverPlugins.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Box".into(),
        attrs: vec![("flexDirection".into(), Value::String("column".into()))]
            .into_iter()
            .collect(),
        children: vec![
            VNode::Empty,
            VNode::Element {
                tag: "Box".into(),
                attrs: vec![].into_iter().collect(),
                children: vec![
                    VNode::Empty,
                    VNode::Element {
                        tag: "Text".into(),
                        attrs: vec![("bold".into(), Value::Bool(true))]
                            .into_iter()
                            .collect(),
                        children: vec![VNode::Text("Discover plugins".into())],
                    },
                    VNode::Empty,
                    VNode::Value(Value::Null),
                    VNode::Empty,
                ],
            },
            VNode::Empty,
            VNode::Empty,
            VNode::Empty,
            VNode::Element {
                tag: "Box".into(),
                attrs: vec![("marginBottom".into(), serde_json::json!(1))]
                    .into_iter()
                    .collect(),
                children: vec![
                    VNode::Empty,
                    VNode::Element {
                        tag: "SearchBox".into(),
                        attrs: vec![
                            ("query".into(), Value::Null),
                            ("isFocused".into(), Value::Null),
                            ("isTerminalFocused".into(), Value::Null),
                            ("width".into(), Value::Null),
                            ("cursorOffset".into(), Value::Null),
                        ]
                        .into_iter()
                        .collect(),
                        children: vec![],
                    },
                    VNode::Empty,
                ],
            },
            VNode::Empty,
            VNode::Empty,
            VNode::Empty,
            VNode::Value(Value::Null),
            VNode::Empty,
            VNode::Empty,
            VNode::Empty,
            VNode::Value(Value::Null),
            VNode::Empty,
            VNode::Empty,
            VNode::Empty,
            VNode::Value(Value::Null),
            VNode::Empty,
            VNode::Empty,
            VNode::Empty,
            VNode::Value(Value::Null),
            VNode::Empty,
            VNode::Empty,
            VNode::Empty,
            VNode::Value(Value::Null),
            VNode::Empty,
            VNode::Empty,
            VNode::Empty,
            VNode::Value(Value::Null),
            VNode::Empty,
            VNode::Element {
                tag: "DiscoverPluginsKeyHint".into(),
                attrs: vec![
                    ("hasSelection".into(), Value::Null),
                    ("canToggle".into(), Value::Null),
                ]
                .into_iter()
                .collect(),
                children: vec![],
            },
            VNode::Empty,
        ],
    }
}
