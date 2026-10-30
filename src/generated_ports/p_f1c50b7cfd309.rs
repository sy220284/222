//! Native Rust UI structure derived from original: src/components/CoordinatorAgentStatus.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Box".into(),
        attrs: vec![
            ("flexDirection".into(), Value::String("column".into())),
            ("marginTop".into(), serde_json::json!(1)),
        ]
        .into_iter()
        .collect(),
        children: vec![
            VNode::Empty,
            VNode::Element {
                tag: "MainLine".into(),
                attrs: vec![
                    ("isSelected".into(), Value::Null),
                    ("isViewed".into(), Value::Null),
                    ("onClick".into(), Value::Null),
                ]
                .into_iter()
                .collect(),
                children: vec![],
            },
            VNode::Empty,
            VNode::Value(Value::Null),
            VNode::Empty,
        ],
    }
}
