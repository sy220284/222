//! Native Rust port derived from original: src/commands/review/UltrareviewOverageDialog.tsx
//!
//! Extra-usage billing confirmation. State machine: select "proceed"
//! flips to Launching and runs onProceed; a rejected onProceed restores
//! the Select so the user can retry instead of staring at "Launching…"
//! (upstream comment: onDone is never called on failure). Selecting
//! "cancel" and Escape both abort the in-flight signal first — a
//! fire-and-forget launch must not keep running and bill after
//! "cancelled". The Dialog/Select widgets are component-domain views.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde_json::{json, Value};

use crate::generated_ports::ui::VNode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchState {
    Idle,
    Launching,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectValue {
    Proceed,
    Cancel,
}

pub fn parse_select(value: &str) -> Option<SelectValue> {
    match value {
        "proceed" => Some(SelectValue::Proceed),
        "cancel" => Some(SelectValue::Cancel),
        _ => None,
    }
}

/// `handleSelect` core: returns the next launch state and the action to
/// perform. A proceed failure resets to Idle (retry-able) upstream via
/// the promise catch; that transition lives with the caller's async run.
pub fn resolve_select(value: SelectValue, current: LaunchState) -> (LaunchState, SelectAction) {
    match value {
        SelectValue::Proceed => (LaunchState::Launching, SelectAction::Proceed),
        SelectValue::Cancel => (current, SelectAction::Cancel),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectAction {
    Proceed,
    Cancel,
}

/// `handleCancel` semantics: abort in-flight work BEFORE cancelling, so
/// the caller skips side effects (confirmOverage, onDone).
pub const CANCEL_ABORTS_BEFORE_CALLBACK: bool = true;

pub const DIALOG_TITLE: &str = "Ultrareview billing";
pub const DIALOG_COLOR: &str = "background";
pub const NOTICE_TEXT: &str = "Your free ultrareviews for this organization are used. \
Further reviews bill as Extra Usage (pay-per-use).";
pub const LAUNCHING_TEXT: &str = "Launching\u{2026}";

pub const OPTIONS: &[(&str, &str)] = &[
    ("Proceed with Extra Usage billing", "proceed"),
    ("Cancel", "cancel"),
];

pub fn render_dialog(is_launching: bool) -> VNode {
    let body: Vec<VNode> = vec![
        VNode::Element {
            tag: "Text".into(),
            attrs: Default::default(),
            children: vec![VNode::Text(NOTICE_TEXT.into())],
        },
        if is_launching {
            VNode::Element {
                tag: "Text".into(),
                attrs: [("color".into(), json!(DIALOG_COLOR))]
                    .into_iter()
                    .collect(),
                children: vec![VNode::Text(LAUNCHING_TEXT.into())],
            }
        } else {
            VNode::Element {
                tag: "Select".into(),
                attrs: [
                    ("options".into(), options_json()),
                    ("onChange".into(), json!("handleSelect")),
                    ("onCancel".into(), json!("handleCancel")),
                ]
                .into_iter()
                .collect(),
                children: vec![],
            }
        },
    ];
    VNode::Element {
        tag: "Dialog".into(),
        attrs: [
            ("title".into(), json!(DIALOG_TITLE)),
            ("onCancel".into(), json!("handleCancel")),
            ("color".into(), json!(DIALOG_COLOR)),
        ]
        .into_iter()
        .collect(),
        children: vec![VNode::Element {
            tag: "Box".into(),
            attrs: [
                ("flexDirection".into(), json!("column")),
                ("gap".into(), json!(1)),
            ]
            .into_iter()
            .collect(),
            children: body,
        }],
    }
}

fn options_json() -> Value {
    Value::Array(
        OPTIONS
            .iter()
            .map(|(label, value)| json!({ "label": label, "value": value }))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proceed_flips_to_launching() {
        let (state, action) = resolve_select(SelectValue::Proceed, LaunchState::Idle);
        assert_eq!(state, LaunchState::Launching);
        assert_eq!(action, SelectAction::Proceed);
    }

    #[test]
    fn cancel_keeps_state_and_fires_cancel() {
        let (state, action) = resolve_select(SelectValue::Cancel, LaunchState::Launching);
        assert_eq!(state, LaunchState::Launching);
        assert_eq!(action, SelectAction::Cancel);
    }

    #[test]
    fn parse_rejects_unknown() {
        assert_eq!(parse_select("proceed"), Some(SelectValue::Proceed));
        assert_eq!(parse_select("nope"), None);
    }

    #[test]
    fn view_switches_select_to_launching_text() {
        let idle = render_dialog(false);
        let launching = render_dialog(true);
        let dump = |v: &VNode| -> String { format!("{v:?}") };
        assert!(dump(&idle).contains("\"Select\""));
        assert!(!dump(&launching).contains("\"Select\""));
        assert!(dump(&launching).contains("Launching"));
        assert!(dump(&idle).contains("Ultrareview billing"));
    }

    #[test]
    fn options_match_original_labels() {
        assert_eq!(OPTIONS.len(), 2);
        assert_eq!(OPTIONS[0].1, "proceed");
        assert_eq!(OPTIONS[1].1, "cancel");
    }
}
