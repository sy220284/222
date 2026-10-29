//! Rust exports derived from original: src/services/remoteManagedSettings/securityCheck.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SecurityCheckResult {
    #[serde(rename = "approved")]
    approved,
    #[serde(rename = "rejected")]
    rejected,
    #[serde(rename = "no_check_needed")]
    no_check_needed,
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
                        tag: "ManagedSettingsSecurityDialog".into(),
                        attrs: vec![
                            ("settings".into(), Value::Null),
                            ("onAccept".into(), Value::Null),
                            ("onReject".into(), Value::Null),
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
