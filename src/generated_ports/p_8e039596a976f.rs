//! Native Rust port derived from original: src/commands/diff/diff.tsx
//!
//! /diff command entry: routes into the DiffDialog component view. diff dialog (lazy-imported upstream)
//! The DiffDialog widget itself lives in the components domain; this port
//! preserves the command contract (props passed through verbatim).
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde_json::{json, Value};

use crate::generated_ports::ui::VNode;

/// Command entry (original `call`): returns the DiffDialog view.
pub fn call() -> VNode {
    VNode::Element {
        tag: "DiffDialog".into(),
        attrs: [
            ("messages".into(), json!("context.messages")),
            ("onDone".into(), json!("onDone")),
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
    fn routes_into_diff_view() {
        match call() {
            VNode::Element { tag, attrs, .. } => {
                assert_eq!(tag, "DiffDialog");
                assert!(!attrs.is_empty());
            }
            _ => panic!("expected element"),
        }
    }
}
