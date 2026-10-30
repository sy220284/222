//! Native Rust UI structure derived from original: src/components/StatusLine.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Box".into(),
        attrs: vec![
            ("paddingX".into(), Value::Null),
            ("gap".into(), serde_json::json!(2)),
        ]
        .into_iter()
        .collect(),
        children: vec![
            VNode::Empty,
            VNode::Value(if crate::generated_ports::runtime::truthy(&Value::Null) {
                Value::Null
            } else {
                if crate::generated_ports::runtime::truthy(&Value::Null) {
                    Value::Null
                } else {
                    Value::Null
                }
            }),
            VNode::Empty,
        ],
    }
}
