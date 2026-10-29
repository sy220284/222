//! Native Rust UI structure derived from original: src/components/EffortCallout.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "PermissionDialog".into(),
        attrs: vec![(
            "title".into(),
            crate::generated_ports::runtime::property(&Value::Null, "dialogTitle"),
        )]
        .into_iter()
        .collect(),
        children: vec![VNode::Element {
            tag: "Box".into(),
            attrs: vec![
                ("flexDirection".into(), Value::String("column".into())),
                ("paddingX".into(), serde_json::json!(2)),
                ("paddingY".into(), serde_json::json!(1)),
            ]
            .into_iter()
            .collect(),
            children: vec![
                VNode::Value(Value::Null),
                VNode::Value(Value::Null),
                VNode::Element {
                    tag: "Select".into(),
                    attrs: vec![
                        ("options".into(), Value::Null),
                        ("onChange".into(), Value::Null),
                        ("onCancel".into(), Value::Null),
                    ]
                    .into_iter()
                    .collect(),
                    children: vec![],
                },
            ],
        }],
    }
}
