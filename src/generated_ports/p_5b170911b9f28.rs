//! Native Rust UI structure derived from original: src/components/StructuredDiff.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "NoSelect".into(),
        attrs: vec![("fromLeftEdge".into(), Value::Bool(true))]
            .into_iter()
            .collect(),
        children: vec![VNode::Element {
            tag: "RawAnsi".into(),
            attrs: vec![("lines".into(), Value::Null), ("width".into(), Value::Null)]
                .into_iter()
                .collect(),
            children: vec![],
        }],
    }
}
