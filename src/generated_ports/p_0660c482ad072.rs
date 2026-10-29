//! Native Rust UI structure derived from original: src/components/FallbackToolUseRejectedMessage.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "MessageResponse".into(),
        attrs: vec![("height".into(), serde_json::json!(1))]
            .into_iter()
            .collect(),
        children: vec![VNode::Element {
            tag: "InterruptedByUser".into(),
            attrs: vec![].into_iter().collect(),
            children: vec![],
        }],
    }
}
