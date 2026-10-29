//! Native Rust UI structure derived from original: src/components/agents/AgentEditor.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Box".into(),
        attrs: vec![
            ("flexDirection".into(), Value::String("column".into())),
            ("tabIndex".into(), serde_json::json!(0)),
            ("autoFocus".into(), Value::Bool(true)),
            ("onKeyDown".into(), Value::Null),
        ]
        .into_iter()
        .collect(),
        children: vec![
            VNode::Empty,
            VNode::Element {
                tag: "Text".into(),
                attrs: vec![("dimColor".into(), Value::Bool(true))]
                    .into_iter()
                    .collect(),
                children: vec![VNode::Text("Source: ".into()), VNode::Value(Value::Null)],
            },
            VNode::Empty,
            VNode::Element {
                tag: "Box".into(),
                attrs: vec![
                    ("marginTop".into(), serde_json::json!(1)),
                    ("flexDirection".into(), Value::String("column".into())),
                ]
                .into_iter()
                .collect(),
                children: vec![VNode::Empty, VNode::Value(Value::Null), VNode::Empty],
            },
            VNode::Empty,
            VNode::Value(Value::Null),
            VNode::Empty,
        ],
    }
}
