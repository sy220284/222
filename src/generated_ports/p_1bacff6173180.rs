//! Native Rust UI structure derived from original: src/components/WorktreeExitDialog.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Dialog".into(),
        attrs: vec![
            (
                "title".into(),
                Value::String("Exiting worktree session".into()),
            ),
            ("subtitle".into(), Value::Null),
            ("onCancel".into(), Value::Null),
        ]
        .into_iter()
        .collect(),
        children: vec![
            VNode::Empty,
            VNode::Element {
                tag: "Select".into(),
                attrs: vec![
                    ("defaultFocusValue".into(), Value::Null),
                    ("options".into(), Value::Null),
                    ("onChange".into(), Value::Null),
                ]
                .into_iter()
                .collect(),
                children: vec![],
            },
            VNode::Empty,
        ],
    }
}
