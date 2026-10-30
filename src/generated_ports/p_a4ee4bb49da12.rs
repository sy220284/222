//! Native Rust port derived from original: src/commands/hooks/hooks.tsx
//!
//! /hooks entry: telemetry ping, then the HooksConfigMenu fed by tool
//! names resolved from the app-state permission context. getTools and
//! the menu widget are injected / component-domain.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde_json::{json, Value};

use crate::generated_ports::ui::VNode;

/// Entry decision: tool names from the permission context feed the menu.
pub fn hooks_entry(tool_names: &[String]) -> VNode {
    VNode::Element {
        tag: "HooksConfigMenu".into(),
        attrs: [
            ("toolNames".into(), json!(tool_names)),
            ("onExit".into(), json!("onDone")),
            ("telemetry".into(), json!("hooks_command")),
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
    fn menu_gets_tool_names_and_exit() {
        let tools = vec!["Bash".to_string(), "Read".to_string()];
        match hooks_entry(&tools) {
            VNode::Element { tag, attrs, .. } => {
                assert_eq!(tag, "HooksConfigMenu");
                assert!(attrs.iter().any(|(k, v)| k == "onExit" && v == "onDone"));
                assert!(attrs.iter().any(|(k, _)| k == "telemetry"));
            }
            _ => panic!(),
        }
    }
}
