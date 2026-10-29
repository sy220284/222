//! Native Rust UI structure derived from original: src/components/PromptInput/PromptInputFooter.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Group(vec![
        VNode::Empty,
        VNode::Element {
            tag: "Box".into(),
            attrs: vec![
                (
                    "flexDirection".into(),
                    if crate::generated_ports::runtime::truthy(&Value::Null) {
                        Value::String("column".into())
                    } else {
                        Value::String("row".into())
                    },
                ),
                (
                    "justifyContent".into(),
                    if crate::generated_ports::runtime::truthy(&Value::Null) {
                        Value::String("flex-start".into())
                    } else {
                        Value::String("space-between".into())
                    },
                ),
                ("paddingX".into(), serde_json::json!(2)),
                (
                    "gap".into(),
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
                VNode::Empty,
                VNode::Element {
                    tag: "Box".into(),
                    attrs: vec![
                        ("flexDirection".into(), Value::String("column".into())),
                        (
                            "flexShrink".into(),
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
                        VNode::Empty,
                        VNode::Value(Value::Null),
                        VNode::Empty,
                        VNode::Element {
                            tag: "PromptInputFooterLeftSide".into(),
                            attrs: vec![
                                ("exitMessage".into(), Value::Null),
                                ("vimMode".into(), Value::Null),
                                ("mode".into(), Value::Null),
                                ("toolPermissionContext".into(), Value::Null),
                                ("suppressHint".into(), Value::Null),
                                ("isLoading".into(), Value::Null),
                                ("tasksSelected".into(), Value::Null),
                                ("teamsSelected".into(), Value::Null),
                                ("teammateFooterIndex".into(), Value::Null),
                                ("tmuxSelected".into(), Value::Null),
                                ("isPasting".into(), Value::Null),
                                ("isSearching".into(), Value::Null),
                                ("historyQuery".into(), Value::Null),
                                ("setHistoryQuery".into(), Value::Null),
                                ("historyFailedMatch".into(), Value::Null),
                                ("onOpenTasksDialog".into(), Value::Null),
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
                    attrs: vec![
                        ("flexShrink".into(), serde_json::json!(1)),
                        ("gap".into(), serde_json::json!(1)),
                    ]
                    .into_iter()
                    .collect(),
                    children: vec![
                        VNode::Empty,
                        VNode::Value(if crate::generated_ports::runtime::truthy(&Value::Null) {
                            Value::Null
                        } else {
                            Value::Null
                        }),
                        VNode::Empty,
                        VNode::Value(Value::Null),
                        VNode::Empty,
                        VNode::Element {
                            tag: "BridgeStatusIndicator".into(),
                            attrs: vec![("bridgeSelected".into(), Value::Null)]
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
    ])
}
