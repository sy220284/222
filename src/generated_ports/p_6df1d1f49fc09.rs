//! Native Rust UI structure derived from original: src/components/AgentProgressLine.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Group(vec![
        VNode::Element {
            tag: "Text".into(),
            attrs: vec![
                ("bold".into(), Value::Bool(true)),
                ("backgroundColor".into(), Value::Null),
                (
                    "color".into(),
                    if crate::generated_ports::runtime::truthy(&Value::Null) {
                        Value::String("inverseText".into())
                    } else {
                        Value::Null
                    },
                ),
            ]
            .into_iter()
            .collect(),
            children: vec![VNode::Value(Value::Null)],
        },
        VNode::Value(Value::Null),
    ])
}
