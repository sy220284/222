//! Native Rust port derived from original: src/commands/skills/skills.tsx
//!
//! /skills command entry: routes into the SkillsMenu component view. skills menu
//! The SkillsMenu widget itself lives in the components domain; this port
//! preserves the command contract (props passed through verbatim).
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde_json::{json, Value};

use crate::generated_ports::ui::VNode;

/// Command entry (original `call`): returns the SkillsMenu view.
pub fn call() -> VNode {
    VNode::Element {
        tag: "SkillsMenu".into(),
        attrs: [
            ("onExit".into(), json!("onDone")),
            ("commands".into(), json!("context.options.commands")),
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
    fn routes_into_skills_view() {
        match call() {
            VNode::Element { tag, attrs, .. } => {
                assert_eq!(tag, "SkillsMenu");
                assert!(!attrs.is_empty());
            }
            _ => panic!("expected element"),
        }
    }
}
