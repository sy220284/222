//! Native Rust UI structure derived from original: src/components/Spinner/SpinnerAnimationRow.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Box".into(),
        attrs: vec![
            ("ref".into(), Value::Null),
            ("flexDirection".into(), Value::String("row".into())),
            ("flexWrap".into(), Value::String("wrap".into())),
            ("marginTop".into(), serde_json::json!(1)),
            ("width".into(), Value::String("100%".into())),
        ]
        .into_iter()
        .collect(),
        children: vec![
            VNode::Empty,
            VNode::Element {
                tag: "SpinnerGlyph".into(),
                attrs: vec![
                    ("frame".into(), Value::Null),
                    ("messageColor".into(), Value::Null),
                    (
                        "stalledIntensity".into(),
                        if crate::generated_ports::runtime::truthy(&Value::Null) {
                            serde_json::json!(0)
                        } else {
                            Value::Null
                        },
                    ),
                    ("reducedMotion".into(), Value::Null),
                    ("time".into(), Value::Null),
                ]
                .into_iter()
                .collect(),
                children: vec![],
            },
            VNode::Empty,
            VNode::Element {
                tag: "GlimmerMessage".into(),
                attrs: vec![
                    ("message".into(), Value::Null),
                    ("mode".into(), Value::Null),
                    ("messageColor".into(), Value::Null),
                    ("glimmerIndex".into(), Value::Null),
                    ("flashOpacity".into(), Value::Null),
                    ("shimmerColor".into(), Value::Null),
                    (
                        "stalledIntensity".into(),
                        if crate::generated_ports::runtime::truthy(&Value::Null) {
                            serde_json::json!(0)
                        } else {
                            Value::Null
                        },
                    ),
                ]
                .into_iter()
                .collect(),
                children: vec![],
            },
            VNode::Empty,
            VNode::Value(Value::Null),
            VNode::Empty,
        ],
    }
}
