//! Native Rust UI structure derived from original: src/components/StructuredDiffList.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Box".into(),
        attrs: vec![
            ("flexDirection".into(), Value::String("column".into())),
            (
                "key".into(),
                crate::generated_ports::runtime::property(&Value::Null, "newStart"),
            ),
        ]
        .into_iter()
        .collect(),
        children: vec![
            VNode::Empty,
            VNode::Element {
                tag: "StructuredDiff".into(),
                attrs: vec![
                    ("patch".into(), Value::Null),
                    ("dim".into(), Value::Null),
                    ("width".into(), Value::Null),
                    ("filePath".into(), Value::Null),
                    ("firstLine".into(), Value::Null),
                    ("fileContent".into(), Value::Null),
                ]
                .into_iter()
                .collect(),
                children: vec![],
            },
            VNode::Empty,
        ],
    }
}
