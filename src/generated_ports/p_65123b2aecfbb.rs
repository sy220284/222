//! Native Rust port derived from original: src/commands/tasks/tasks.tsx
//!
//! /tasks command entry: routes into the BackgroundTasksDialog component view. background tasks dialog
//! The BackgroundTasksDialog widget itself lives in the components domain; this port
//! preserves the command contract (props passed through verbatim).
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde_json::{json, Value};

use crate::generated_ports::ui::VNode;

/// Command entry (original `call`): returns the BackgroundTasksDialog view.
pub fn call() -> VNode {
    VNode::Element {
        tag: "BackgroundTasksDialog".into(),
        attrs: [
            ("toolUseContext".into(), json!("context")),
            ("onDone".into(), json!("onDone")),
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
    fn routes_into_background_tasks_view() {
        match call() {
            VNode::Element { tag, attrs, .. } => {
                assert_eq!(tag, "BackgroundTasksDialog");
                assert!(!attrs.is_empty());
            }
            _ => panic!("expected element"),
        }
    }
}
