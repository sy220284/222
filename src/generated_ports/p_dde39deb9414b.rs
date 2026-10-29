//! Native Rust UI structure derived from original: src/components/AwsAuthStatusBox.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Box".into(),
        attrs: vec![
            ("flexDirection".into(), Value::String("column".into())),
            ("borderStyle".into(), Value::String("round".into())),
            ("borderColor".into(), Value::String("permission".into())),
            ("paddingX".into(), serde_json::json!(1)),
            ("marginY".into(), serde_json::json!(1)),
        ]
        .into_iter()
        .collect(),
        children: vec![
            VNode::Value(Value::Null),
            VNode::Value(Value::Null),
            VNode::Value(Value::Null),
        ],
    }
}
