//! Native Rust UI structure derived from original: src/components/Settings/Config.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Box".into(),
        attrs: vec![
            ("flexDirection".into(), Value::String("column".into())),
            ("width".into(), Value::String("100%".into())),
            ("tabIndex".into(), serde_json::json!(0)),
            ("autoFocus".into(), Value::Bool(true)),
            ("onKeyDown".into(), Value::Null),
        ]
        .into_iter()
        .collect(),
        children: vec![
            VNode::Empty,
            VNode::Value(if crate::generated_ports::runtime::truthy(&Value::Null) {
                Value::Null
            } else {
                if crate::generated_ports::runtime::truthy(&Value::Null) {
                    Value::Null
                } else {
                    if crate::generated_ports::runtime::truthy(&Value::Null) {
                        Value::Null
                    } else {
                        if crate::generated_ports::runtime::truthy(&Value::Null) {
                            Value::Null
                        } else {
                            if crate::generated_ports::runtime::truthy(&Value::Null) {
                                Value::Null
                            } else {
                                if crate::generated_ports::runtime::truthy(&Value::Null) {
                                    Value::Null
                                } else {
                                    if crate::generated_ports::runtime::truthy(&Value::Null) {
                                        Value::Null
                                    } else {
                                        if crate::generated_ports::runtime::truthy(&Value::Null) {
                                            Value::Null
                                        } else {
                                            Value::Null
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }),
            VNode::Empty,
        ],
    }
}
