//! Native Rust UI structure derived from original: src/components/PromptInput/HistorySearchInput.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "TextInput".into(),
        attrs: vec![
            ("value".into(), Value::Null),
            ("onChange".into(), Value::Null),
            (
                "cursorOffset".into(),
                crate::generated_ports::runtime::property(&Value::Null, "length"),
            ),
            ("onChangeCursorOffset".into(), Value::Null),
            ("columns".into(), Value::Null),
            ("focus".into(), Value::Bool(true)),
            ("showCursor".into(), Value::Bool(true)),
            ("multiline".into(), Value::Bool(false)),
            ("dimColor".into(), Value::Bool(true)),
        ]
        .into_iter()
        .collect(),
        children: vec![],
    }
}
