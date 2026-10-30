//! Native Rust UI structure derived from original: src/components/agents/ToolSelector.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "React.Fragment".into(),
        attrs: vec![(
            "key".into(),
            crate::generated_ports::runtime::property(&Value::Null, "id"),
        )]
        .into_iter()
        .collect(),
        children: vec![
            VNode::Value(Value::Null),
            VNode::Value(Value::Null),
            VNode::Element {
                tag: "Text".into(),
                attrs: vec![
                    (
                        "color".into(),
                        if crate::generated_ports::runtime::truthy(&Value::Null) {
                            Value::Null
                        } else {
                            if crate::generated_ports::runtime::truthy(&Value::Null) {
                                Value::String("suggestion".into())
                            } else {
                                Value::Null
                            }
                        },
                    ),
                    ("dimColor".into(), Value::Null),
                    ("bold".into(), Value::Null),
                ]
                .into_iter()
                .collect(),
                children: vec![
                    VNode::Value(if crate::generated_ports::runtime::truthy(&Value::Null) {
                        Value::String("".into())
                    } else {
                        if crate::generated_ports::runtime::truthy(&Value::Null) {
                            Value::Null
                        } else {
                            Value::String("  ".into())
                        }
                    }),
                    VNode::Value(if crate::generated_ports::runtime::truthy(&Value::Null) {
                        Value::Null
                    } else {
                        crate::generated_ports::runtime::property(&Value::Null, "label")
                    }),
                ],
            },
        ],
    }
}
