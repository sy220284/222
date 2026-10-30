//! Native Rust port derived from original: src/components/agents/AgentNavigationFooter.tsx
//!
//! Agent nav footer text decision: custom instructions override the
//! default navigation hint; an exit-pending state replaces both with
//! "Press {key} again to exit". The ctrl-c/ctrl-d hook is component domain.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use crate::generated_ports::ui::VNode;

pub const DEFAULT_NAV_HINT: &str = "Press ↑↓ to navigate · Enter to select · Esc to go back";

/// Footer text decision (original `t2` computation).
pub fn footer_text(instructions: Option<&str>, exit_pending: bool, exit_key: &str) -> String {
    if exit_pending {
        return format!("Press {exit_key} again to exit");
    }
    instructions.unwrap_or(DEFAULT_NAV_HINT).to_string()
}

pub fn render_footer(text: &str) -> VNode {
    VNode::Element {
        tag: "Box".into(),
        attrs: [
            ("marginLeft".into(), "2".into()),
            ("dimColor".into(), "true".into()),
        ]
        .into_iter()
        .collect(),
        children: vec![VNode::Text(text.into())],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_pending_overrides_instructions() {
        assert_eq!(
            footer_text(Some("custom"), true, "ctrl+c"),
            "Press ctrl+c again to exit"
        );
        assert_eq!(footer_text(None, false, ""), DEFAULT_NAV_HINT);
        assert_eq!(footer_text(Some("custom"), false, ""), "custom");
    }
}
