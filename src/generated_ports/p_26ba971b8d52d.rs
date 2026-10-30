//! Native Rust port derived from original: src/commands/install-github-app/CheckGitHubStep.tsx
//!
//! Trivial static step: the GitHub CLI availability check message.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use crate::generated_ports::ui::VNode;

pub fn checking_message() -> &'static str {
    "Checking GitHub CLI installation\u{2026}"
}

pub fn render_source_view() -> VNode {
    VNode::Element {
        tag: "Text".into(),
        attrs: Default::default(),
        children: vec![VNode::Text(checking_message().into())],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_matches_original_text() {
        assert_eq!(
            checking_message(),
            "Checking GitHub CLI installation\u{2026}"
        );
    }
}
