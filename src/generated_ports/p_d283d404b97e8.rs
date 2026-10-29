//! Rust exports derived from original: src/components/FeedbackSurvey/TranscriptSharePrompt.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TranscriptShareResponse {
    #[serde(rename = "yes")]
    yes,
    #[serde(rename = "no")]
    no,
    #[serde(rename = "dont_ask_again")]
    dont_ask_again,
}

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
            VNode::Value(Value::Null),
            VNode::Value(Value::Null),
            VNode::Element {
                tag: "Box".into(),
                attrs: vec![("marginLeft".into(), serde_json::json!(2))]
                    .into_iter()
                    .collect(),
                children: vec![
                    VNode::Value(Value::Null),
                    VNode::Value(Value::Null),
                    VNode::Element {
                        tag: "Box".into(),
                        attrs: vec![].into_iter().collect(),
                        children: vec![VNode::Element {
                            tag: "Text".into(),
                            attrs: vec![].into_iter().collect(),
                            children: vec![
                                VNode::Element {
                                    tag: "Text".into(),
                                    attrs: vec![(
                                        "color".into(),
                                        Value::String("ansi:cyan".into()),
                                    )]
                                    .into_iter()
                                    .collect(),
                                    children: vec![VNode::Text("3".into())],
                                },
                                VNode::Text(": Don't ask again".into()),
                            ],
                        }],
                    },
                ],
            },
        ],
    }
}
