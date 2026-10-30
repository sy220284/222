//! Native Rust port derived from original: src/commands/agents/agents.tsx
//!
//! /agents command entry: routes into the AgentsMenu component view. agents menu
//! The AgentsMenu widget itself lives in the components domain; this port
//! preserves the command contract (props passed through verbatim).
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde_json::{json, Value};

use crate::generated_ports::ui::VNode;

/// Command entry (original `call`): returns the AgentsMenu view.
pub fn call() -> VNode {
    VNode::Element {
        tag: "AgentsMenu".into(),
        attrs: [
            ("tools".into(), json!("getTools(permissionContext)")),
            ("onExit".into(), json!("onDone")),
        ]
        .into_iter()
        .collect(),
        children: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_into_agents_view() {
        match call() {
            VNode::Element { tag, attrs, .. } => {
                assert_eq!(tag, "AgentsMenu");
                assert!(!attrs.is_empty());
            }
            _ => panic!("expected element"),
        }
    }
}
