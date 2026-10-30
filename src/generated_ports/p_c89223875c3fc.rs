//! Native Rust UI structure derived from original: src/components/InvalidConfigDialog.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "AppStateProvider".into(),
        attrs: vec![].into_iter().collect(),
        children: vec![
            VNode::Empty,
            VNode::Element {
                tag: "KeybindingSetup".into(),
                attrs: vec![].into_iter().collect(),
                children: vec![
                    VNode::Empty,
                    VNode::Element {
                        tag: "InvalidConfigDialog".into(),
                        attrs: vec![
                            (
                                "filePath".into(),
                                crate::generated_ports::runtime::property(&Value::Null, "filePath"),
                            ),
                            (
                                "errorDescription".into(),
                                crate::generated_ports::runtime::property(&Value::Null, "message"),
                            ),
                            ("onExit".into(), Value::Null),
                            ("onReset".into(), Value::Null),
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
