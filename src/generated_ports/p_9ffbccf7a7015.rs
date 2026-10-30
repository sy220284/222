//! Native Rust UI structure derived from original: src/components/FullscreenLayout.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "ModalContext".into(),
        attrs: vec![("value".into(), Value::Null)].into_iter().collect(),
        children: vec![VNode::Element {
            tag: "Box".into(),
            attrs: vec![
                ("position".into(), Value::String("absolute".into())),
                ("bottom".into(), serde_json::json!(0)),
                ("left".into(), serde_json::json!(0)),
                ("right".into(), serde_json::json!(0)),
                ("maxHeight".into(), Value::Null),
                ("flexDirection".into(), Value::String("column".into())),
                ("overflow".into(), Value::String("hidden".into())),
                ("opaque".into(), Value::Bool(true)),
            ]
            .into_iter()
            .collect(),
            children: vec![
                VNode::Element {
                    tag: "Box".into(),
                    attrs: vec![("flexShrink".into(), serde_json::json!(0))]
                        .into_iter()
                        .collect(),
                    children: vec![VNode::Element {
                        tag: "Text".into(),
                        attrs: vec![("color".into(), Value::String("permission".into()))]
                            .into_iter()
                            .collect(),
                        children: vec![VNode::Value(Value::Null)],
                    }],
                },
                VNode::Element {
                    tag: "Box".into(),
                    attrs: vec![
                        ("flexDirection".into(), Value::String("column".into())),
                        ("paddingX".into(), serde_json::json!(2)),
                        ("flexShrink".into(), serde_json::json!(0)),
                        ("overflow".into(), Value::String("hidden".into())),
                    ]
                    .into_iter()
                    .collect(),
                    children: vec![VNode::Value(Value::Null)],
                },
            ],
        }],
    }
}
