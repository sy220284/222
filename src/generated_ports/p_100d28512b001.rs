//! Native Rust UI structure derived from original: src/components/PromptInput/PromptInputFooterLeftSide.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Box".into(),
        attrs: vec![
            ("height".into(), serde_json::json!(1)),
            ("overflow".into(), Value::String("hidden".into())),
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
        ],
    }
}
