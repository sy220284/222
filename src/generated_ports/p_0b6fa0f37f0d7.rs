//! Native Rust UI structure derived from original: src/components/ResumeTask.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Box".into(),
        attrs: vec![
            ("flexDirection".into(), Value::String("column".into())),
            ("padding".into(), serde_json::json!(1)),
            ("height".into(), Value::Null),
        ]
        .into_iter()
        .collect(),
        children: vec![
            VNode::Empty,
            VNode::Element {
                tag: "Text".into(),
                attrs: vec![("bold".into(), Value::Bool(true))]
                    .into_iter()
                    .collect(),
                children: vec![
                    VNode::Text("Select a session to resume\n        ".into()),
                    VNode::Value(Value::Null),
                    VNode::Empty,
                    VNode::Value(Value::Null),
                    VNode::Text(":\n      ".into()),
                ],
            },
            VNode::Empty,
            VNode::Element {
                tag: "Box".into(),
                attrs: vec![
                    ("flexDirection".into(), Value::String("column".into())),
                    ("marginTop".into(), serde_json::json!(1)),
                    ("flexGrow".into(), serde_json::json!(1)),
                ]
                .into_iter()
                .collect(),
                children: vec![
                    VNode::Empty,
                    VNode::Element {
                        tag: "Box".into(),
                        attrs: vec![("marginLeft".into(), serde_json::json!(2))]
                            .into_iter()
                            .collect(),
                        children: vec![
                            VNode::Empty,
                            VNode::Element {
                                tag: "Text".into(),
                                attrs: vec![("bold".into(), Value::Bool(true))]
                                    .into_iter()
                                    .collect(),
                                children: vec![
                                    VNode::Empty,
                                    VNode::Value(Value::Null),
                                    VNode::Empty,
                                    VNode::Value(Value::Null),
                                    VNode::Empty,
                                    VNode::Value(Value::String("Session Title".into())),
                                    VNode::Empty,
                                ],
                            },
                            VNode::Empty,
                        ],
                    },
                    VNode::Empty,
                    VNode::Element {
                        tag: "Select".into(),
                        attrs: vec![
                            ("visibleOptionCount".into(), Value::Null),
                            ("options".into(), Value::Null),
                            ("onChange".into(), Value::Null),
                            ("onFocus".into(), Value::Null),
                        ]
                        .into_iter()
                        .collect(),
                        children: vec![],
                    },
                    VNode::Empty,
                ],
            },
            VNode::Empty,
            VNode::Element {
                tag: "Box".into(),
                attrs: vec![("flexDirection".into(), Value::String("row".into()))]
                    .into_iter()
                    .collect(),
                children: vec![
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
                                            ("action".into(), Value::String("select".into())),
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
                                            ("action".into(), Value::String("confirm".into())),
                                        ]
                                        .into_iter()
                                        .collect(),
                                        children: vec![],
                                    },
                                    VNode::Empty,
                                    VNode::Element {
                                        tag: "ConfigurableShortcutHint".into(),
                                        attrs: vec![
                                            ("action".into(), Value::String("confirm:no".into())),
                                            (
                                                "context".into(),
                                                Value::String("Confirmation".into()),
                                            ),
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
                    },
                    VNode::Empty,
                ],
            },
            VNode::Empty,
        ],
    }
}
