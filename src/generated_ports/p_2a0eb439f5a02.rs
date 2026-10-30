//! Native Rust port derived from original: src/commands/review/ultrareviewCommand.tsx
//!
//! /ultrareview gate orchestration: overage gate four-way branch
//! (not-enabled / low-balance / needs-confirm dialog / proceed), abort
//! semantics for the ~5s launch (user Escape during launch skips onDone
//! — it would write to a dead transcript slot — and skips persisting
//! the confirmation flag so the dialog re-shows next attempt), and the
//! launch-failure message for teleport/non-github-repo connectivity
//! issues. contentBlocksToString is a verbatim pure function.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde_json::{json, Value};

use crate::generated_ports::ui::VNode;

/// Joins text content blocks with newlines (verbatim core).
pub fn content_blocks_to_string(blocks: &[Value]) -> String {
    blocks
        .iter()
        .filter_map(|b| {
            if b.get("type").and_then(|v| v.as_str()) == Some("text") {
                b.get("text").and_then(|v| v.as_str())
            } else {
                None
            }
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Debug, Clone, PartialEq)]
pub enum OverageGate {
    NotEnabled,
    LowBalance { available: f64 },
    NeedsConfirm,
    Proceed { billing_note: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum GateOutcome {
    Message(&'static str),
    LowBalanceMessage(String),
    ShowDialog,
    Launch,
}

/// Gate four-way branch (original `call` head).
pub fn route_gate(gate: &OverageGate) -> GateOutcome {
    match gate {
        OverageGate::NotEnabled => GateOutcome::Message(
            "Free ultrareviews used. Enable Extra Usage at https://upstream.example/settings/billing to continue.",
        ),
        OverageGate::LowBalance { available } => GateOutcome::LowBalanceMessage(format!(
            "Balance too low to launch ultrareview (${available:.2} available, $10 minimum). Top up at https://upstream.example/settings/billing"
        )),
        OverageGate::NeedsConfirm => GateOutcome::ShowDialog,
        OverageGate::Proceed { .. } => GateOutcome::Launch,
    }
}

/// `launchAndDone` completion semantics: an aborted launch returns None
/// (dialog already unmounted, onDone skipped); a successful launch maps
/// content blocks; null result is a connectivity failure message.
pub fn launch_completion(aborted: bool, result: Option<&[Value]>) -> Option<(String, bool)> {
    if aborted {
        return None;
    }
    match result {
        Some(blocks) if !blocks.is_empty() => {
            Some((content_blocks_to_string(blocks), true))
        }
        _ => Some((
            "Ultrareview failed to launch the remote session. Check that this is a GitHub repo and try again.".into(),
            false,
        )),
    }
}

/// Confirmation flag persistence rule: only after a non-aborted launch.
pub fn should_confirm_overage(aborted: bool) -> bool {
    !aborted
}

pub fn render_overage_dialog() -> VNode {
    VNode::Element {
        tag: "UltrareviewOverageDialog".into(),
        attrs: [
            ("onProceed".into(), json!("launchAndDone")),
            ("onCancel".into(), json!("cancelled")),
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
    fn content_blocks_join_text_only() {
        let blocks = vec![
            json!({"type": "text", "text": "line1"}),
            json!({"type": "image"}),
            json!({"type": "text", "text": "line2"}),
        ];
        assert_eq!(content_blocks_to_string(&blocks), "line1\nline2");
    }

    #[test]
    fn gate_branches_route_correctly() {
        assert_eq!(
            route_gate(&OverageGate::NotEnabled),
            GateOutcome::Message("Free ultrareviews used. Enable Extra Usage at https://upstream.example/settings/billing to continue.")
        );
        assert!(matches!(
            route_gate(&OverageGate::LowBalance { available: 3.5 }),
            GateOutcome::LowBalanceMessage(_)
        ));
        assert_eq!(
            route_gate(&OverageGate::NeedsConfirm),
            GateOutcome::ShowDialog
        );
        assert!(matches!(
            route_gate(&OverageGate::Proceed {
                billing_note: "n".into()
            }),
            GateOutcome::Launch
        ));
    }

    #[test]
    fn aborted_launch_skips_on_done_and_flag() {
        assert!(launch_completion(true, None).is_none());
        assert!(!should_confirm_overage(true));
        assert!(should_confirm_overage(false));
    }

    #[test]
    fn completion_maps_result_or_connectivity_error() {
        let blocks = vec![json!({"type": "text", "text": "review done"})];
        assert_eq!(
            launch_completion(false, Some(&blocks)),
            Some(("review done".into(), true))
        );
        let (msg, should_query) = launch_completion(false, None).unwrap();
        assert!(msg.contains("failed to launch the remote session"));
        assert!(!should_query);
    }

    #[test]
    fn dialog_view_present() {
        match render_overage_dialog() {
            VNode::Element { tag, .. } => assert_eq!(tag, "UltrareviewOverageDialog"),
            _ => panic!(),
        }
    }
}
