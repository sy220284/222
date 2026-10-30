//! Native Rust UI structure derived from original: src/commands/plugin/PluginOptionsDialog.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Dialog".into(),
        attrs: vec![
            ("title".into(), Value::Null),
            ("subtitle".into(), Value::Null),
            ("onCancel".into(), Value::Null),
            ("isCancelActive".into(), Value::Bool(false)),
        ]
        .into_iter()
        .collect(),
        children: vec![VNode::Value(Value::Null), VNode::Value(Value::Null)],
    }
}
