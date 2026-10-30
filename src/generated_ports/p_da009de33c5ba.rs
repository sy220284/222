//! Rust exports derived from original: src/components/permissions/PermissionPrompt.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FeedbackType {
    #[serde(rename = "accept")]
    accept,
    #[serde(rename = "reject")]
    reject,
}

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Select".into(),
        attrs: vec![
            ("options".into(), Value::Null),
            ("inlineDescriptions".into(), Value::Bool(true)),
            ("onChange".into(), Value::Null),
            ("onCancel".into(), Value::Null),
            ("onFocus".into(), Value::Null),
            ("onInputModeToggle".into(), Value::Null),
        ]
        .into_iter()
        .collect(),
        children: vec![],
    }
}
