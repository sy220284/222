//! Native Rust UI structure derived from original: src/components/Settings/Usage.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Box".into(),
        attrs: vec![
            ("flexDirection".into(), Value::String("column".into())),
            ("gap".into(), serde_json::json!(1)),
            ("width".into(), Value::String("100%".into())),
        ]
        .into_iter()
        .collect(),
        children: vec![
            VNode::Empty,
            VNode::Value(Value::Null),
            VNode::Empty,
            VNode::Value(Value::Null),
            VNode::Empty,
            VNode::Value(Value::Null),
            VNode::Empty,
            VNode::Value(Value::Null),
            VNode::Empty,
            VNode::Element {
                tag: "Text".into(),
                attrs: vec![("dimColor".into(), Value::Bool(true))]
                    .into_iter()
                    .collect(),
                children: vec![
                    VNode::Empty,
                    VNode::Element {
                        tag: "ConfigurableShortcutHint".into(),
                        attrs: vec![
                            ("action".into(), Value::String("confirm:no".into())),
                            ("context".into(), Value::String("Settings".into())),
                            ("fallback".into(), Value::String("Esc".into())),
                            ("description".into(), Value::String("cancel".into())),
                        ]
                        .into_iter()
                        .collect(),
                        children: vec![],
                    },
                    VNode::Empty,
                ],
            },
            VNode::Empty,
        ],
    }
}
