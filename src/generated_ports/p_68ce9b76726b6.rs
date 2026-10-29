//! Native Rust UI structure derived from original: src/components/ContextSuggestions.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Box".into(),
        attrs: vec![
            ("key".into(), Value::Null),
            ("flexDirection".into(), Value::String("column".into())),
            (
                "marginTop".into(),
                if crate::generated_ports::runtime::truthy(&Value::Null) {
                    serde_json::json!(0)
                } else {
                    serde_json::json!(1)
                },
            ),
        ]
        .into_iter()
        .collect(),
        children: vec![
            VNode::Element {
                tag: "Box".into(),
                attrs: vec![].into_iter().collect(),
                children: vec![
                    VNode::Element {
                        tag: "StatusIcon".into(),
                        attrs: vec![
                            (
                                "status".into(),
                                crate::generated_ports::runtime::property(&Value::Null, "severity"),
                            ),
                            ("withSpace".into(), Value::Bool(true)),
                        ]
                        .into_iter()
                        .collect(),
                        children: vec![],
                    },
                    VNode::Element {
                        tag: "Text".into(),
                        attrs: vec![("bold".into(), Value::Bool(true))]
                            .into_iter()
                            .collect(),
                        children: vec![VNode::Value(crate::generated_ports::runtime::property(
                            &Value::Null,
                            "title",
                        ))],
                    },
                    VNode::Value(
                        if crate::generated_ports::runtime::truthy(
                            &crate::generated_ports::runtime::property(
                                &Value::Null,
                                "savingsTokens",
                            ),
                        ) {
                            Value::Null
                        } else {
                            Value::Null
                        },
                    ),
                ],
            },
            VNode::Element {
                tag: "Box".into(),
                attrs: vec![("marginLeft".into(), serde_json::json!(2))]
                    .into_iter()
                    .collect(),
                children: vec![VNode::Element {
                    tag: "Text".into(),
                    attrs: vec![("dimColor".into(), Value::Bool(true))]
                        .into_iter()
                        .collect(),
                    children: vec![VNode::Value(crate::generated_ports::runtime::property(
                        &Value::Null,
                        "detail",
                    ))],
                }],
            },
        ],
    }
}
