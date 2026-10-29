//! Native Rust UI structure derived from original: src/components/IdeStatusIndicator.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Text".into(),
        attrs: vec![
            ("color".into(), Value::String("ide".into())),
            ("key".into(), Value::String("selection-indicator".into())),
            ("wrap".into(), Value::String("truncate".into())),
        ]
        .into_iter()
        .collect(),
        children: vec![
            VNode::Text("⧉ ".into()),
            VNode::Value(crate::generated_ports::runtime::property(
                &Value::Null,
                "lineCount",
            )),
            VNode::Value(Value::String(" ".into())),
            VNode::Value(Value::Null),
            VNode::Text("selected".into()),
        ],
    }
}
