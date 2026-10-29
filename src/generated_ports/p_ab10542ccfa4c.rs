//! Rust exports derived from original: src/buddy/CompanionSprite.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub fn MIN_COLS_FOR_FULL_SPRITE() -> Value {
    serde_json::from_str(r#"100"#).expect("Valid source constant")
}

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Box".into(),
        attrs: vec![
            ("paddingX".into(), serde_json::json!(1)),
            ("alignSelf".into(), Value::String("flex-end".into())),
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
                    VNode::Value(Value::Null),
                    VNode::Empty,
                    VNode::Element {
                        tag: "Text".into(),
                        attrs: vec![
                            ("bold".into(), Value::Bool(true)),
                            ("color".into(), Value::Null),
                        ]
                        .into_iter()
                        .collect(),
                        children: vec![VNode::Empty, VNode::Value(Value::Null), VNode::Empty],
                    },
                    VNode::Value(Value::String(" ".into())),
                    VNode::Empty,
                    VNode::Element {
                        tag: "Text".into(),
                        attrs: vec![
                            ("italic".into(), Value::Bool(true)),
                            ("dimColor".into(), Value::Null),
                            ("bold".into(), Value::Null),
                            ("inverse".into(), Value::Null),
                            (
                                "color".into(),
                                if crate::generated_ports::runtime::truthy(&Value::Null) {
                                    if crate::generated_ports::runtime::truthy(&Value::Null) {
                                        Value::String("inactive".into())
                                    } else {
                                        Value::Null
                                    }
                                } else {
                                    if crate::generated_ports::runtime::truthy(&Value::Null) {
                                        Value::Null
                                    } else {
                                        Value::Null
                                    }
                                },
                            ),
                        ]
                        .into_iter()
                        .collect(),
                        children: vec![VNode::Empty, VNode::Value(Value::Null), VNode::Empty],
                    },
                    VNode::Empty,
                ],
            },
            VNode::Empty,
        ],
    }
}
