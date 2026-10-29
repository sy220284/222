//! Native Rust UI structure derived from original: src/components/ManagedSettingsSecurityDialog/ManagedSettingsSecurityDialog.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element{tag:"Text".into(),attrs:vec![].into_iter().collect(),children:vec![VNode::Text("Your organization has configured managed settings that could allow execution of arbitrary code or interception of your prompts and responses.".into())]}
}
