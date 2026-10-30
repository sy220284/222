//! Native Rust UI structure derived from original: src/components/VirtualMessageList.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Group(vec![
        VNode::Empty,
        VNode::Element {
            tag: "Box".into(),
            attrs: vec![
                ("ref".into(), Value::Null),
                ("height".into(), Value::Null),
                ("flexShrink".into(), serde_json::json!(0)),
            ]
            .into_iter()
            .collect(),
            children: vec![],
        },
        VNode::Empty,
        VNode::Value(Value::Null),
        VNode::Empty,
        VNode::Value(Value::Null),
        VNode::Empty,
        VNode::Value(Value::Null),
        VNode::Empty,
    ])
}
