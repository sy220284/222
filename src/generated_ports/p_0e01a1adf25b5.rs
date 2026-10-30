//! Native Rust UI structure derived from original: src/components/DevBar.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Text".into(),
        attrs: vec![
            ("wrap".into(), Value::String("truncate-end".into())),
            ("color".into(), Value::String("warning".into())),
        ]
        .into_iter()
        .collect(),
        children: vec![
            VNode::Text("[ANT-ONLY] slow sync: ".into()),
            VNode::Value(Value::Null),
        ],
    }
}
