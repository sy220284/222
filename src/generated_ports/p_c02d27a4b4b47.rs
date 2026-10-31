//! Native Rust port derived from original: src/components/HelpV2/General.tsx
//!
//! Help general tab: the verbatim intro copy plus the Shortcuts section
//! (bold heading, help menu with gap 2 and fixed width).
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use crate::generated_ports::ui::VNode;

pub const GENERAL_INTRO: &str = "Helps with your codebase, makes edits with your permission, and executes commands — right from your terminal.";

pub fn general_help_view() -> VNode {
    VNode::Element {
        tag: "Box".into(),
        attrs: [].into_iter().collect(),
        children: vec![
            VNode::Text(GENERAL_INTRO.into()),
            VNode::Element {
                tag: "Shortcuts".into(),
                attrs: [
                    ("gap".into(), "2".into()),
                    ("fixedWidth".into(), "true".into()),
                ]
                .into_iter()
                .collect(),
                children: vec![VNode::Text("Shortcuts".into())],
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intro_copy_and_shortcuts_section() {
        match general_help_view() {
            VNode::Element { children, .. } => {
                assert_eq!(children.len(), 2);
                assert!(
                    matches!(&children[0], VNode::Text(t) if t.contains("right from your terminal"))
                );
                match &children[1] {
                    VNode::Element { tag, attrs, .. } => {
                        assert_eq!(tag, "Shortcuts");
                        assert!(attrs.iter().any(|(k, v)| k == "fixedWidth" && v == "true"));
                    }
                    _ => panic!(),
                }
            }
            _ => panic!(),
        }
    }
}
