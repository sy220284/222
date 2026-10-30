//! Native Rust UI structure derived from original: src/components/LogoV2/Opus1mMergeNotice.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Box".into(),
        attrs: vec![("paddingLeft".into(), serde_json::json!(2))]
            .into_iter()
            .collect(),
        children: vec![
            VNode::Element {
                tag: "AnimatedAsterisk".into(),
                attrs: vec![("char".into(), Value::Null)].into_iter().collect(),
                children: vec![],
            },
            VNode::Element {
                tag: "Text".into(),
                attrs: vec![("dimColor".into(), Value::Bool(true))]
                    .into_iter()
                    .collect(),
                children: vec![
                    VNode::Value(Value::String(" ".into())),
                    VNode::Text(
                        "Opus now defaults to 1M context · 5x more room, same pricing".into(),
                    ),
                ],
            },
        ],
    }
}
