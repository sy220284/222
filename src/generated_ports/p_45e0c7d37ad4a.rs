//! Native Rust port derived from original: src/commands/btw/btw.tsx
//!
//! /btw side-question overlay: keyboard contract and fetch branching as
//! pure decision functions. Chrome metrics are verbatim (5 inner rows,
//! 6 outer, 3-line scroll steps). The React state machine, ScrollBox and
//! runSideQuestion service call are component/domain-internal.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde_json::{json, Value};

use crate::generated_ports::ui::VNode;

pub const CHROME_ROWS: usize = 5;
pub const OUTER_CHROME_ROWS: usize = 6;
pub const SCROLL_LINES: isize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    DoneSkip,
    ScrollUp,
    ScrollDown,
    Ignore,
}

/// `handleKeyDown` contract: escape/return/space/ctrl-c/ctrl-d finish
/// with display "skip"; up/ctrl-p scrolls up; down/ctrl-n scrolls down.
pub fn key_action(key: &str, ctrl: bool) -> KeyAction {
    match (key, ctrl) {
        ("escape", _) | ("return", _) | (" ", _) => KeyAction::DoneSkip,
        ("c", true) | ("d", true) => KeyAction::DoneSkip,
        ("up", _) | ("p", true) => KeyAction::ScrollUp,
        ("down", _) | ("n", true) => KeyAction::ScrollDown,
        _ => KeyAction::Ignore,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum FetchOutcome {
    Responded(String),
    Empty,
    Failed(String),
}

/// Fetch branching: a response string wins; a null/empty response is
/// "No response received"; errors keep their message or the default.
pub fn fetch_outcome(result: Option<&str>, error: Option<&str>) -> FetchOutcome {
    if let Some(response) = result.filter(|s| !s.is_empty()) {
        return FetchOutcome::Responded(response.to_string());
    }
    if let Some(err) = error {
        if err.is_empty() {
            return FetchOutcome::Failed("Failed to get response".into());
        }
        return FetchOutcome::Failed(err.to_string());
    }
    FetchOutcome::Empty
}

pub fn render() -> VNode {
    VNode::Element {
        tag: "BtwSideQuestion".into(),
        attrs: [("spinnerFrameMs".into(), json!(80))].into_iter().collect(),
        children: vec![],
    }
}

/// Registry view shim.
pub fn btwChromeRows(_args: &[Value]) -> Value {
    json!(CHROME_ROWS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dismiss_keys_finish_with_skip() {
        for key in ["escape", "return", " "] {
            assert_eq!(key_action(key, false), KeyAction::DoneSkip);
        }
        assert_eq!(key_action("c", true), KeyAction::DoneSkip);
        assert_eq!(key_action("d", true), KeyAction::DoneSkip);
    }

    #[test]
    fn scroll_keys_and_ignored() {
        assert_eq!(key_action("up", false), KeyAction::ScrollUp);
        assert_eq!(key_action("p", true), KeyAction::ScrollUp);
        assert_eq!(key_action("down", false), KeyAction::ScrollDown);
        assert_eq!(key_action("n", true), KeyAction::ScrollDown);
        assert_eq!(key_action("x", false), KeyAction::Ignore);
    }

    #[test]
    fn fetch_branching() {
        assert_eq!(
            fetch_outcome(Some("answer"), None),
            FetchOutcome::Responded("answer".into())
        );
        assert_eq!(fetch_outcome(None, None), FetchOutcome::Empty);
        assert_eq!(
            fetch_outcome(None, Some("rate limited")),
            FetchOutcome::Failed("rate limited".into())
        );
        assert_eq!(
            fetch_outcome(None, Some("")),
            FetchOutcome::Failed("Failed to get response".into())
        );
    }

    #[test]
    fn chrome_metrics_verbatim() {
        assert_eq!(CHROME_ROWS, 5);
        assert_eq!(OUTER_CHROME_ROWS, 6);
        assert_eq!(SCROLL_LINES, 3);
    }
}
