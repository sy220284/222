//! Native Rust UI structure derived from original: src/components/PackageManagerAutoUpdater.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Text".into(),
        attrs: vec![
            ("color".into(), Value::String("warning".into())),
            ("wrap".into(), Value::String("truncate".into())),
        ]
        .into_iter()
        .collect(),
        children: vec![
            VNode::Text("Update available! Run: ".into()),
            VNode::Element {
                tag: "Text".into(),
                attrs: vec![("bold".into(), Value::Bool(true))]
                    .into_iter()
                    .collect(),
                children: vec![VNode::Value(Value::Null)],
            },
        ],
    }
}
