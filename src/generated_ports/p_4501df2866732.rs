//! Native Rust UI structure derived from original: src/components/MemoryUsageIndicator.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Box".into(),
        attrs: vec![].into_iter().collect(),
        children: vec![
            VNode::Empty,
            VNode::Element {
                tag: "Text".into(),
                attrs: vec![
                    ("color".into(), Value::Null),
                    ("wrap".into(), Value::String("truncate".into())),
                ]
                .into_iter()
                .collect(),
                children: vec![
                    VNode::Text("High memory usage (".into()),
                    VNode::Value(Value::Null),
                    VNode::Text(") · /heapdump\n      ".into()),
                ],
            },
            VNode::Empty,
        ],
    }
}
