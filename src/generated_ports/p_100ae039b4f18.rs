//! Native Rust UI structure derived from original: src/commands/plugin/AddMarketplace.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Box".into(),
        attrs: vec![("flexDirection".into(), Value::String("column".into()))]
            .into_iter()
            .collect(),
        children: vec![
            VNode::Empty,
            VNode::Element {
                tag: "Box".into(),
                attrs: vec![
                    ("flexDirection".into(), Value::String("column".into())),
                    ("paddingX".into(), serde_json::json!(1)),
                    ("borderStyle".into(), Value::String("round".into())),
                ]
                .into_iter()
                .collect(),
                children: vec![
                    VNode::Empty,
                    VNode::Element {
                        tag: "Box".into(),
                        attrs: vec![("marginBottom".into(), serde_json::json!(1))]
                            .into_iter()
                            .collect(),
                        children: vec![
                            VNode::Empty,
                            VNode::Element {
                                tag: "Text".into(),
                                attrs: vec![("bold".into(), Value::Bool(true))]
                                    .into_iter()
                                    .collect(),
                                children: vec![VNode::Text("Add Marketplace".into())],
                            },
                            VNode::Empty,
                        ],
                    },
                    VNode::Empty,
                    VNode::Element {
                        tag: "Box".into(),
                        attrs: vec![("flexDirection".into(), Value::String("column".into()))]
                            .into_iter()
                            .collect(),
                        children: vec![
                            VNode::Empty,
                            VNode::Element {
                                tag: "Text".into(),
                                attrs: vec![].into_iter().collect(),
                                children: vec![VNode::Text("Enter marketplace source:".into())],
                            },
                            VNode::Empty,
                            VNode::Element {
                                tag: "Text".into(),
                                attrs: vec![("dimColor".into(), Value::Bool(true))]
                                    .into_iter()
                                    .collect(),
                                children: vec![VNode::Text("Examples:".into())],
                            },
                            VNode::Empty,
                            VNode::Element {
                                tag: "Text".into(),
                                attrs: vec![("dimColor".into(), Value::Bool(true))]
                                    .into_iter()
                                    .collect(),
                                children: vec![VNode::Text("· owner/repo (GitHub)".into())],
                            },
                            VNode::Empty,
                            VNode::Element {
                                tag: "Text".into(),
                                attrs: vec![("dimColor".into(), Value::Bool(true))]
                                    .into_iter()
                                    .collect(),
                                children: vec![VNode::Text(
                                    "· git@github.com:owner/repo.git (SSH)".into(),
                                )],
                            },
                            VNode::Empty,
                            VNode::Element {
                                tag: "Text".into(),
                                attrs: vec![("dimColor".into(), Value::Bool(true))]
                                    .into_iter()
                                    .collect(),
                                children: vec![VNode::Text(
                                    "· https://example.com/marketplace.json".into(),
                                )],
                            },
                            VNode::Empty,
                            VNode::Element {
                                tag: "Text".into(),
                                attrs: vec![("dimColor".into(), Value::Bool(true))]
                                    .into_iter()
                                    .collect(),
                                children: vec![VNode::Text("· ./path/to/marketplace".into())],
                            },
                            VNode::Empty,
                            VNode::Element {
                                tag: "Box".into(),
                                attrs: vec![("marginTop".into(), serde_json::json!(1))]
                                    .into_iter()
                                    .collect(),
                                children: vec![
                                    VNode::Empty,
                                    VNode::Element {
                                        tag: "TextInput".into(),
                                        attrs: vec![
                                            ("value".into(), Value::Null),
                                            ("onChange".into(), Value::Null),
                                            ("onSubmit".into(), Value::Null),
                                            ("columns".into(), serde_json::json!(80)),
                                            ("cursorOffset".into(), Value::Null),
                                            ("onChangeCursorOffset".into(), Value::Null),
                                            ("focus".into(), Value::Bool(true)),
                                            ("showCursor".into(), Value::Bool(true)),
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
                    VNode::Value(Value::Null),
                    VNode::Empty,
                    VNode::Value(Value::Null),
                    VNode::Empty,
                    VNode::Value(Value::Null),
                    VNode::Empty,
                ],
            },
            VNode::Empty,
            VNode::Element {
                tag: "Box".into(),
                attrs: vec![("marginLeft".into(), serde_json::json!(3))]
                    .into_iter()
                    .collect(),
                children: vec![
                    VNode::Empty,
                    VNode::Element {
                        tag: "Text".into(),
                        attrs: vec![
                            ("dimColor".into(), Value::Bool(true)),
                            ("italic".into(), Value::Bool(true)),
                        ]
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
                                            ("shortcut".into(), Value::String("Enter".into())),
                                            ("action".into(), Value::String("add".into())),
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
                    },
                    VNode::Empty,
                ],
            },
            VNode::Empty,
        ],
    }
}
