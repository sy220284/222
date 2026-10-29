//! Native Rust UI structure derived from original: src/components/FallbackToolUseErrorMessage.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Box".into(),
        attrs: vec![].into_iter().collect(),
        children: vec![
            VNode::Element {
                tag: "Text".into(),
                attrs: vec![("dimColor".into(), Value::Bool(true))]
                    .into_iter()
                    .collect(),
                children: vec![
                    VNode::Text("… +".into()),
                    VNode::Value(Value::Null),
                    VNode::Empty,
                    VNode::Value(if crate::generated_ports::runtime::truthy(&Value::Null) {
                        Value::String("line".into())
                    } else {
                        Value::String("lines".into())
                    }),
                    VNode::Text("(".into()),
                ],
            },
            VNode::Element {
                tag: "Text".into(),
                attrs: vec![
                    ("dimColor".into(), Value::Bool(true)),
                    ("bold".into(), Value::Bool(true)),
                ]
                .into_iter()
                .collect(),
                children: vec![VNode::Value(Value::Null)],
            },
            VNode::Element {
                tag: "Text".into(),
                attrs: vec![].into_iter().collect(),
                children: vec![VNode::Empty],
            },
            VNode::Element {
                tag: "Text".into(),
                attrs: vec![("dimColor".into(), Value::Bool(true))]
                    .into_iter()
                    .collect(),
                children: vec![VNode::Text("to see all)".into())],
            },
        ],
    }
}
