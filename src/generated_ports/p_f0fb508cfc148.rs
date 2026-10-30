//! Native Rust UI structure derived from original: src/commands/permissions/permissions.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "PermissionRuleList".into(),
        attrs: vec![
            ("onExit".into(), Value::Null),
            ("onRetryDenials".into(), Value::Null),
        ]
        .into_iter()
        .collect(),
        children: vec![],
    }
}
