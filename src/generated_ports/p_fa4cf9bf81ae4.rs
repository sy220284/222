//! Native Rust port derived from original: src/commands/extra-usage/extra-usage.tsx
//!
//! /extra-usage orchestration: `runExtraUsage` returns either a message
//! (surfaced via onDone) or a login flow — a Login view with the
//! upstream starting message, completing as "Login successful" or
//! "Login interrupted" and notifying the API-key change. The Login
//! widget and runExtraUsage service are injected; this port keeps the
//! branch semantics and the exact strings.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde_json::{json, Value};

use crate::generated_ports::ui::VNode;

pub const STARTING_MESSAGE: &str =
    "Starting new login following /extra-usage. Exit with Ctrl-C to use existing account.";

#[derive(Debug, Clone, PartialEq)]
pub enum ExtraUsageOutcome {
    /// runExtraUsage produced a message: surface and stop.
    Message(String),
    /// Login flow view with completion messages.
    LoginView,
}

pub fn orchestrate(run_result: Result<String, ()>) -> ExtraUsageOutcome {
    match run_result {
        Ok(value) => ExtraUsageOutcome::Message(value),
        Err(()) => ExtraUsageOutcome::LoginView,
    }
}

pub fn login_completion_message(success: bool) -> &'static str {
    if success {
        "Login successful"
    } else {
        "Login interrupted"
    }
}

pub fn render_login_view() -> VNode {
    VNode::Element {
        tag: "Login".into(),
        attrs: [
            ("startingMessage".into(), json!(STARTING_MESSAGE)),
            ("onDone".into(), json!("success => onChangeAPIKey + onDone")),
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
    fn message_branch_surfaces_value() {
        assert_eq!(
            orchestrate(Ok("quota ok".into())),
            ExtraUsageOutcome::Message("quota ok".into())
        );
    }

    #[test]
    fn login_branch_uses_upstream_starting_message() {
        assert_eq!(orchestrate(Err(())), ExtraUsageOutcome::LoginView);
        match render_login_view() {
            VNode::Element { attrs, .. } => assert!(attrs
                .iter()
                .any(|(k, v)| k == "startingMessage" && v.as_str().unwrap().contains("Ctrl-C"))),
            _ => panic!(),
        }
    }

    #[test]
    fn completion_messages() {
        assert_eq!(login_completion_message(true), "Login successful");
        assert_eq!(login_completion_message(false), "Login interrupted");
    }
}
