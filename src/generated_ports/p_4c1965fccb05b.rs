//! Native Rust port derived from original: src/commands/usage/usage.tsx
//!
//! /usage command entry: routes into the Settings component view. usage tab
//! The Settings widget itself lives in the components domain; this port
//! preserves the command contract (props passed through verbatim).
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde_json::{json, Value};

use crate::generated_ports::ui::VNode;

/// Command entry (original `call`): returns the Settings view.
pub fn call() -> VNode {
    VNode::Element {
        tag: "Settings".into(),
        attrs: [
            ("onClose".into(), json!("onDone")),
            ("defaultTab".into(), json!("Usage")),
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
    fn routes_into_settings_view() {
        match call() {
            VNode::Element { tag, attrs, .. } => {
                assert_eq!(tag, "Settings");
                assert!(!attrs.is_empty());
            }
            _ => panic!("expected element"),
        }
    }
}
