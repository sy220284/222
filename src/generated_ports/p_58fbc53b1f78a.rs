//! Native Rust UI structure derived from original: src/components/Feedback.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Dialog".into(),
        attrs: vec![
            (
                "title".into(),
                Value::String("Submit Feedback / Bug Report".into()),
            ),
            ("onCancel".into(), Value::Null),
            ("isCancelActive".into(), Value::Null),
            ("inputGuide".into(), Value::Null),
        ]
        .into_iter()
        .collect(),
        children: vec![
            VNode::Empty,
            VNode::Value(Value::Null),
            VNode::Empty,
            VNode::Value(Value::Null),
            VNode::Empty,
            VNode::Value(Value::Null),
            VNode::Empty,
            VNode::Value(Value::Null),
            VNode::Empty,
        ],
    }
}
