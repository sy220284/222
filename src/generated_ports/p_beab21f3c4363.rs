//! Native Rust UI structure derived from original: src/commands/fast/fast.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Group(vec![
        VNode::Element {
            tag: "Box".into(),
            attrs: vec![
                ("flexDirection".into(), Value::String("column".into())),
                ("gap".into(), serde_json::json!(0)),
                ("marginLeft".into(), serde_json::json!(2)),
            ]
            .into_iter()
            .collect(),
            children: vec![VNode::Element {
                tag: "Box".into(),
                attrs: vec![
                    ("flexDirection".into(), Value::String("row".into())),
                    ("gap".into(), serde_json::json!(2)),
                ]
                .into_iter()
                .collect(),
                children: vec![
                    VNode::Element {
                        tag: "Text".into(),
                        attrs: vec![("bold".into(), Value::Bool(true))]
                            .into_iter()
                            .collect(),
                        children: vec![VNode::Text("Fast mode".into())],
                    },
                    VNode::Element {
                        tag: "Text".into(),
                        attrs: vec![
                            (
                                "color".into(),
                                if crate::generated_ports::runtime::truthy(&Value::Null) {
                                    Value::String("fastMode".into())
                                } else {
                                    Value::Null
                                },
                            ),
                            ("bold".into(), Value::Null),
                        ]
                        .into_iter()
                        .collect(),
                        children: vec![VNode::Value(
                            if crate::generated_ports::runtime::truthy(&Value::Null) {
                                Value::String("ON ".into())
                            } else {
                                Value::String("OFF".into())
                            },
                        )],
                    },
                    VNode::Element {
                        tag: "Text".into(),
                        attrs: vec![("dimColor".into(), Value::Bool(true))]
                            .into_iter()
                            .collect(),
                        children: vec![VNode::Value(Value::Null)],
                    },
                ],
            }],
        },
        VNode::Value(Value::Null),
    ])
}
