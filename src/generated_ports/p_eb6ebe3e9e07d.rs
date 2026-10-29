//! Native Rust UI structure derived from original: src/components/LogoV2/EmergencyTip.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Box".into(),
        attrs: vec![
            ("paddingLeft".into(), serde_json::json!(2)),
            ("flexDirection".into(), Value::String("column".into())),
        ]
        .into_iter()
        .collect(),
        children: vec![
            VNode::Empty,
            VNode::Element {
                tag: "Text".into(),
                attrs: vec![].into_iter().collect(),
                children: vec![
                    VNode::Empty,
                    VNode::Value(crate::generated_ports::runtime::property(
                        &Value::Null,
                        "tip",
                    )),
                    VNode::Empty,
                ],
            },
            VNode::Empty,
        ],
    }
}
