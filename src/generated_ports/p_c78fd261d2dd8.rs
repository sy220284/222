//! Native Rust UI structure derived from original: src/components/design-system/FuzzyPicker.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Pane".into(),
        attrs: vec![("color".into(), Value::String("permission".into()))]
            .into_iter()
            .collect(),
        children: vec![
            VNode::Empty,
            VNode::Element {
                tag: "Box".into(),
                attrs: vec![
                    ("flexDirection".into(), Value::String("column".into())),
                    ("gap".into(), serde_json::json!(1)),
                    ("tabIndex".into(), serde_json::json!(0)),
                    ("autoFocus".into(), Value::Bool(true)),
                    ("onKeyDown".into(), Value::Null),
                ]
                .into_iter()
                .collect(),
                children: vec![
                    VNode::Empty,
                    VNode::Element {
                        tag: "Text".into(),
                        attrs: vec![
                            ("bold".into(), Value::Bool(true)),
                            ("color".into(), Value::String("permission".into())),
                        ]
                        .into_iter()
                        .collect(),
                        children: vec![VNode::Empty, VNode::Value(Value::Null), VNode::Empty],
                    },
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
                                tag: "Byline".into(),
                                attrs: vec![].into_iter().collect(),
                                children: vec![
                                    VNode::Empty,
                                    VNode::Element {
                                        tag: "KeyboardShortcutHint".into(),
                                        attrs: vec![
                                            ("shortcut".into(), Value::String("↑/↓".into())),
                                            (
                                                "action".into(),
                                                if crate::generated_ports::runtime::truthy(
                                                    &Value::Null,
                                                ) {
                                                    Value::String("nav".into())
                                                } else {
                                                    Value::String("navigate".into())
                                                },
                                            ),
                                        ]
                                        .into_iter()
                                        .collect(),
                                        children: vec![],
                                    },
                                    VNode::Empty,
                                    VNode::Element {
                                        tag: "KeyboardShortcutHint".into(),
                                        attrs: vec![
                                            ("shortcut".into(), Value::String("Enter".into())),
                                            (
                                                "action".into(),
                                                if crate::generated_ports::runtime::truthy(
                                                    &Value::Null,
                                                ) {
                                                    Value::Null
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
                                    VNode::Value(Value::Null),
                                    VNode::Empty,
                                    VNode::Element {
                                        tag: "KeyboardShortcutHint".into(),
                                        attrs: vec![
                                            ("shortcut".into(), Value::String("Esc".into())),
                                            ("action".into(), Value::String("cancel".into())),
                                        ]
                                        .into_iter()
                                        .collect(),
                                        children: vec![],
                                    },
                                    VNode::Empty,
                                    VNode::Value(Value::Null),
                                    VNode::Empty,
                                ],
                            },
                            VNode::Empty,
                        ],
                    },
                    VNode::Empty,
                ],
            },
            VNode::Empty,
        ],
    }
}
