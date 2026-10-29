//! Rust exports derived from original: src/utils/teleport.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TeleportProgressStep {
    #[serde(rename = "validating")]
    validating,
    #[serde(rename = "fetching_logs")]
    fetching_logs,
    #[serde(rename = "fetching_branch")]
    fetching_branch,
    #[serde(rename = "checking_out")]
    checking_out,
    #[serde(rename = "done")]
    done,
}

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
                        tag: "TeleportError".into(),
                        attrs: vec![
                            ("errorsToIgnore".into(), Value::Null),
                            ("onComplete".into(), Value::Null),
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
