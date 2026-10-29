//! Native Rust UI structure derived from original: src/commands/plugin/BrowseMarketplace.tsx
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
                        children: vec![VNode::Text("Plugin Details".into())],
                    },
                    VNode::Empty,
                ],
            },
            VNode::Empty,
            VNode::Empty,
            VNode::Empty,
            VNode::Element {
                tag: "Box".into(),
                attrs: vec![
                    ("flexDirection".into(), Value::String("column".into())),
                    ("marginBottom".into(), serde_json::json!(1)),
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
                        children: vec![VNode::Value(crate::generated_ports::runtime::property(
                            &crate::generated_ports::runtime::property(&Value::Null, "entry"),
                            "name",
                        ))],
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
            VNode::Empty,
            VNode::Empty,
            VNode::Element {
                tag: "Box".into(),
                attrs: vec![
                    ("flexDirection".into(), Value::String("column".into())),
                    ("marginBottom".into(), serde_json::json!(1)),
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
                        children: vec![VNode::Text("Will install:".into())],
                    },
                    VNode::Empty,
                    VNode::Value(Value::Null),
                    VNode::Empty,
                    VNode::Value(Value::Null),
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
                tag: "PluginTrustWarning".into(),
                attrs: vec![].into_iter().collect(),
                children: vec![],
            },
            VNode::Empty,
            VNode::Empty,
            VNode::Empty,
            VNode::Value(Value::Null),
            VNode::Empty,
            VNode::Empty,
            VNode::Empty,
            VNode::Element {
                tag: "Box".into(),
                attrs: vec![("flexDirection".into(), Value::String("column".into()))]
                    .into_iter()
                    .collect(),
                children: vec![VNode::Empty, VNode::Value(Value::Null), VNode::Empty],
            },
            VNode::Empty,
            VNode::Element {
                tag: "Box".into(),
                attrs: vec![
                    ("marginTop".into(), serde_json::json!(1)),
                    ("paddingLeft".into(), serde_json::json!(1)),
                ]
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
                                        tag: "ConfigurableShortcutHint".into(),
                                        attrs: vec![
                                            (
                                                "action".into(),
                                                Value::String("select:accept".into()),
                                            ),
                                            ("context".into(), Value::String("Select".into())),
                                            ("fallback".into(), Value::String("Enter".into())),
                                            ("description".into(), Value::String("select".into())),
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
                                            ("description".into(), Value::String("back".into())),
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
