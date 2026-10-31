//! Native Rust port derived from original: src/components/FallbackToolUseRejectedMessage.tsx
//!
//! Fallback rejection view: InterruptedByUser inside a MessageResponse
//! with height 1. Structure preserved; child widgets are component-domain.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use crate::generated_ports::ui::VNode;

pub fn fallback_tool_use_rejected_message() -> VNode {
    VNode::Element {
        tag: "MessageResponse".into(),
        attrs: [("height".into(), "1".into())].into_iter().collect(),
        children: vec![VNode::Element {
            tag: "InterruptedByUser".into(),
            attrs: [].into_iter().collect(),
            children: vec![],
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structure_preserved() {
        match fallback_tool_use_rejected_message() {
            VNode::Element {
                tag,
                attrs,
                children,
            } => {
                assert_eq!(tag, "MessageResponse");
                assert!(attrs.iter().any(|(k, v)| k == "height" && v == "1"));
                assert_eq!(children.len(), 1);
                match &children[0] {
                    VNode::Element { tag, .. } => assert_eq!(tag, "InterruptedByUser"),
                    _ => panic!(),
                }
            }
            _ => panic!(),
        }
    }
}
