//! Native Rust port derived from original: src/utils/autoRunIssue.tsx
//!
//! Auto-run /issue notification. The keybinding `confirm:no` drives
//! cancel; the auto-run effect fires exactly once (ref-guarded). The
//! `shouldAutoRunIssue` gate is verbatim: non-ant user types never
//! auto-run, and every listed reason returns false (upstream structure
//! preserved for parity). The ANT-ONLY command resolver maps external
//! users to None.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde_json::{json, Value};

use crate::generated_ports::ui::VNode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoRunIssueReason {
    FeedbackSurveyBad,
    FeedbackSurveyGood,
}

/// Notification view: bold running header, dim Esc hint, dim reason line.
pub fn render_auto_run_notification(reason: &str) -> VNode {
    VNode::Element {
        tag: "Box".into(),
        attrs: [
            ("flexDirection".into(), json!("column")),
            ("marginTop".into(), json!(1)),
        ]
        .into_iter()
        .collect(),
        children: vec![
            VNode::Element {
                tag: "Box".into(),
                attrs: Default::default(),
                children: vec![VNode::Element {
                    tag: "Text".into(),
                    attrs: [("bold".into(), json!(true))].into_iter().collect(),
                    children: vec![VNode::Text("Running feedback capture...".into())],
                }],
            },
            VNode::Element {
                tag: "Box".into(),
                attrs: Default::default(),
                children: vec![VNode::Element {
                    tag: "Text".into(),
                    attrs: [("dimColor".into(), json!(true))].into_iter().collect(),
                    children: vec![VNode::Text(
                        "Press \u{1b}[36mEsc\u{1b}[0m anytime (cancel)".into(),
                    )],
                }],
            },
            VNode::Element {
                tag: "Box".into(),
                attrs: Default::default(),
                children: vec![VNode::Element {
                    tag: "Text".into(),
                    attrs: [("dimColor".into(), json!(true))].into_iter().collect(),
                    children: vec![VNode::Text(format!("Reason: {reason}"))],
                }],
            },
        ],
    }
}

/// Verbatim gate: external user types never auto-run and every listed
/// reason returns false.
pub fn should_auto_run_issue(_reason: AutoRunIssueReason) -> bool {
    // Original: if ("external" !== 'ant') return false; — the production
    // user type is external, so the gate is always false. Structure kept.
    false
}

/// ANT-ONLY command resolver: external user types have no auto-run command.
pub fn get_auto_run_issue_command(_reason: AutoRunIssueReason) -> Option<&'static str> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_is_verbatim_false() {
        assert!(!should_auto_run_issue(
            AutoRunIssueReason::FeedbackSurveyBad
        ));
        assert!(!should_auto_run_issue(
            AutoRunIssueReason::FeedbackSurveyGood
        ));
    }

    #[test]
    fn ant_only_command_absent_for_external() {
        assert!(get_auto_run_issue_command(AutoRunIssueReason::FeedbackSurveyGood).is_none());
    }

    #[test]
    fn notification_carries_reason_and_header() {
        match render_auto_run_notification("feedback_survey_bad") {
            VNode::Element { children, .. } => {
                assert_eq!(children.len(), 3);
                let third = &children[2];
                if let VNode::Element {
                    children: inner, ..
                } = third
                {
                    if let VNode::Element { children: leaf, .. } = &inner[0] {
                        if let VNode::Text(t) = &leaf[0] {
                            assert!(t.contains("Reason: feedback_survey_bad"));
                        }
                    }
                }
            }
            _ => panic!("expected element"),
        }
    }
}
