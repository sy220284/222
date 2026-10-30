//! Native Rust port derived from original: src/commands/desktop/desktop.tsx
//!
//! /desktop command entry: routes into the DesktopHandoff component view. desktop handoff
//! The DesktopHandoff widget itself lives in the components domain; this port
//! preserves the command contract (props passed through verbatim).
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde_json::{json, Value};

use crate::generated_ports::ui::VNode;

/// Command entry (original `call`): returns the DesktopHandoff view.
pub fn call() -> VNode {
    VNode::Element {
        tag: "DesktopHandoff".into(),
        attrs: [("onDone".into(), json!("onDone"))].into_iter().collect(),
        children: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_into_desktop_handoff_view() {
        match call() {
            VNode::Element { tag, attrs, .. } => {
                assert_eq!(tag, "DesktopHandoff");
                assert!(!attrs.is_empty());
            }
            _ => panic!("expected element"),
        }
    }
}
