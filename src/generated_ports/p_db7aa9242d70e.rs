//! Native Rust port derived from original: src/commands/feedback/feedback.tsx
//!
//! /feedback entry: routes args as the initial bug description into the
//! Feedback component view (abort signal, transcript messages and
//! background-task summaries pass through). The Feedback widget lives
//! in the components domain; this port keeps the command contract and
//! the arg-defaulting behavior.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde_json::Value;

use crate::generated_ports::ui::VNode;

/// Entry contract: `args` defaults to an empty description.
pub fn feedback_initial_description(args: Option<&str>) -> String {
    args.unwrap_or("").to_string()
}

/// Routes the command into the Feedback component view with the
/// upstream prop set (onDone/abort signal/messages/backgroundTasks are
/// provided by the caller's runtime).
pub fn call(args: Option<&str>, messages: Value) -> VNode {
    VNode::Element {
        tag: "Feedback".into(),
        attrs: [
            (
                "initialDescription".into(),
                Value::String(feedback_initial_description(args)),
            ),
            ("messages".into(), messages),
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
    fn args_become_initial_description() {
        assert_eq!(
            feedback_initial_description(Some("login broken")),
            "login broken"
        );
        assert_eq!(feedback_initial_description(None), "");
    }

    #[test]
    fn call_routes_into_feedback_view() {
        let view = call(Some("x"), serde_json::json!([]));
        match view {
            VNode::Element { tag, attrs, .. } => {
                assert_eq!(tag, "Feedback");
                assert!(attrs.contains_key("initialDescription"));
                assert!(attrs.contains_key("messages"));
            }
            _ => panic!("expected element"),
        }
    }
}
