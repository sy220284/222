//! Native Rust UI structure derived from original: src/commands/rate-limit-options/rate-limit-options.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Dialog".into(),
        attrs: vec![
            (
                "title".into(),
                Value::String("What do you want to do?".into()),
            ),
            ("onCancel".into(), Value::Null),
            ("color".into(), Value::String("suggestion".into())),
        ]
        .into_iter()
        .collect(),
        children: vec![VNode::Value(Value::Null)],
    }
}
