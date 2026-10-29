//! Native Rust UI structure derived from original: src/commands/hooks/hooks.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "HooksConfigMenu".into(),
        attrs: vec![
            ("toolNames".into(), Value::Null),
            ("onExit".into(), Value::Null),
        ]
        .into_iter()
        .collect(),
        children: vec![],
    }
}
