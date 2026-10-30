//! Native Rust UI structure derived from original: src/commands/usage/usage.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Settings".into(),
        attrs: vec![
            ("onClose".into(), Value::Null),
            ("context".into(), Value::Null),
            ("defaultTab".into(), Value::String("Usage".into())),
        ]
        .into_iter()
        .collect(),
        children: vec![],
    }
}
