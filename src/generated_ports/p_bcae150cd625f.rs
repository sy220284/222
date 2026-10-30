//! Native Rust UI structure derived from original: src/commands/plan/plan.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Box".into(),
        attrs: vec![("marginTop".into(), serde_json::json!(1))]
            .into_iter()
            .collect(),
        children: vec![
            VNode::Element {
                tag: "Text".into(),
                attrs: vec![("dimColor".into(), Value::Bool(true))]
                    .into_iter()
                    .collect(),
                children: vec![VNode::Text("\"/plan open\"".into())],
            },
            VNode::Element {
                tag: "Text".into(),
                attrs: vec![("dimColor".into(), Value::Bool(true))]
                    .into_iter()
                    .collect(),
                children: vec![VNode::Text("to edit this plan in ".into())],
            },
            VNode::Element {
                tag: "Text".into(),
                attrs: vec![
                    ("bold".into(), Value::Bool(true)),
                    ("dimColor".into(), Value::Bool(true)),
                ]
                .into_iter()
                .collect(),
                children: vec![VNode::Value(Value::Null)],
            },
        ],
    }
}
