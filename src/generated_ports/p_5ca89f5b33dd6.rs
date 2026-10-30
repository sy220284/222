//! Native Rust port derived from original: src/components/PromptInput/IssueFlagBanner.tsx
//!
//! ANT-ONLY banner prompting issue reports via /issue when conversation
//! friction is detected. External builds render null — preserved as the
//! gate result. FLAG_ICON view is component-domain.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use crate::generated_ports::ui::VNode;

/// Friction-gate: ant builds show the banner, external builds null.
pub fn issue_flag_banner(is_ant: bool, friction_detected: bool) -> Option<VNode> {
    if is_ant && friction_detected {
        Some(VNode::Element {
            tag: "IssueFlagBanner".into(),
            attrs: [("/issue".into(), "FLAG_ICON".into())]
                .into_iter()
                .collect(),
            children: vec![],
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_builds_render_null() {
        assert!(issue_flag_banner(false, true).is_none());
    }

    #[test]
    fn ant_builds_require_friction() {
        assert!(issue_flag_banner(true, false).is_none());
        assert!(issue_flag_banner(true, true).is_some());
    }
}

/// Registry-view wrapper preserving the original export name.
pub fn IssueFlagBanner(_args: &[serde_json::Value]) -> serde_json::Value {
    serde_json::json!(null)
}
