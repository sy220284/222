//! Rust exports derived from original: src/components/StructuredDiff/Fallback.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineObject {
    #[serde(rename = "code")]
    pub code: String,
    #[serde(rename = "i")]
    pub i: f64,
    #[serde(rename = "type")]
    pub type_: Value,
    #[serde(rename = "originalCode")]
    pub originalCode: String,
    #[serde(rename = "wordDiff", default)]
    pub wordDiff: Option<bool>,
    #[serde(rename = "matchedLine", default)]
    pub matchedLine: Option<Value>,
}

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Box".into(),
        attrs: vec![
            ("key".into(), Value::Null),
            ("flexDirection".into(), Value::String("row".into())),
        ]
        .into_iter()
        .collect(),
        children: vec![
            VNode::Empty,
            VNode::Element {
                tag: "NoSelect".into(),
                attrs: vec![("fromLeftEdge".into(), Value::Bool(true))]
                    .into_iter()
                    .collect(),
                children: vec![
                    VNode::Empty,
                    VNode::Element {
                        tag: "Text".into(),
                        attrs: vec![
                            (
                                "color".into(),
                                if crate::generated_ports::runtime::truthy(&Value::Null) {
                                    Value::String("text".into())
                                } else {
                                    Value::Null
                                },
                            ),
                            ("backgroundColor".into(), Value::Null),
                            ("dimColor".into(), Value::Null),
                        ]
                        .into_iter()
                        .collect(),
                        children: vec![
                            VNode::Empty,
                            VNode::Value(Value::Null),
                            VNode::Empty,
                            VNode::Value(Value::Null),
                            VNode::Empty,
                        ],
                    },
                    VNode::Empty,
                ],
            },
            VNode::Empty,
            VNode::Element {
                tag: "Text".into(),
                attrs: vec![
                    (
                        "color".into(),
                        if crate::generated_ports::runtime::truthy(&Value::Null) {
                            Value::String("text".into())
                        } else {
                            Value::Null
                        },
                    ),
                    ("backgroundColor".into(), Value::Null),
                    ("dimColor".into(), Value::Null),
                ]
                .into_iter()
                .collect(),
                children: vec![
                    VNode::Empty,
                    VNode::Value(Value::Null),
                    VNode::Empty,
                    VNode::Value(Value::Null),
                    VNode::Empty,
                ],
            },
            VNode::Empty,
        ],
    }
}
