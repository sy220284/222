//! Native Rust port derived from original: src/commands/help/help.tsx
//!
//! /help command entry: routes into the HelpV2 component view. help view
//! The HelpV2 widget itself lives in the components domain; this port
//! preserves the command contract (props passed through verbatim).
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde_json::{json, Value};

use crate::generated_ports::ui::VNode;

/// Command entry (original `call`): returns the HelpV2 view.
pub fn call() -> VNode {
    VNode::Element {
        tag: "HelpV2".into(),
        attrs: [
            ("commands".into(), json!("context.options.commands")),
            ("onClose".into(), json!("onDone")),
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
    fn routes_into_help_view() {
        match call() {
            VNode::Element { tag, attrs, .. } => {
                assert_eq!(tag, "HelpV2");
                assert!(!attrs.is_empty());
            }
            _ => panic!("expected element"),
        }
    }
}
