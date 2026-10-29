//! Native Rust UI structure derived from original: src/components/SessionPreview.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Messages".into(),
        attrs: vec![
            (
                "messages".into(),
                crate::generated_ports::runtime::property(&Value::Null, "messages"),
            ),
            ("tools".into(), Value::Null),
            ("commands".into(), Value::Null),
            ("verbose".into(), Value::Bool(true)),
            ("toolJSX".into(), Value::Null),
            ("toolUseConfirmQueue".into(), Value::Null),
            ("inProgressToolUseIDs".into(), Value::Null),
            ("isMessageSelectorVisible".into(), Value::Bool(false)),
            ("conversationId".into(), Value::Null),
            ("screen".into(), Value::String("transcript".into())),
            ("streamingToolUses".into(), Value::Null),
            ("showAllInTranscript".into(), Value::Bool(true)),
            ("isLoading".into(), Value::Bool(false)),
        ]
        .into_iter()
        .collect(),
        children: vec![],
    }
}
