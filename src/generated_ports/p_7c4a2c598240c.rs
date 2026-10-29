//! Native Rust UI structure derived from original: src/commands/install-github-app/ChooseRepoStep.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Box".into(),
        attrs: vec![
            ("marginLeft".into(), serde_json::json!(2)),
            ("marginBottom".into(), serde_json::json!(1)),
        ]
        .into_iter()
        .collect(),
        children: vec![VNode::Element {
            tag: "TextInput".into(),
            attrs: vec![
                ("value".into(), Value::Null),
                ("onChange".into(), Value::Null),
                ("onSubmit".into(), Value::Null),
                ("focus".into(), Value::Bool(true)),
                (
                    "placeholder".into(),
                    Value::String(
                        "Enter a repo as owner/repo or https://github.com/owner/repo…".into(),
                    ),
                ),
                ("columns".into(), Value::Null),
                ("cursorOffset".into(), Value::Null),
                ("onChangeCursorOffset".into(), Value::Null),
                ("showCursor".into(), Value::Bool(true)),
            ]
            .into_iter()
            .collect(),
            children: vec![],
        }],
    }
}
