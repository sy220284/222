//! Rust exports derived from original: src/components/PromptInput/PromptInputFooterSuggestions.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SuggestionType {
    #[serde(rename = "command")]
    command,
    #[serde(rename = "file")]
    file,
    #[serde(rename = "directory")]
    directory,
    #[serde(rename = "agent")]
    agent,
    #[serde(rename = "shell")]
    shell,
    #[serde(rename = "custom-title")]
    custom_title,
    #[serde(rename = "slack-channel")]
    slack_channel,
    #[serde(rename = "none")]
    none,
}

pub fn OVERLAY_MAX_ITEMS() -> Value {
    serde_json::from_str(r#"5"#).expect("Valid source constant")
}

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "SuggestionItemRow".into(),
        attrs: vec![
            (
                "key".into(),
                crate::generated_ports::runtime::property(&Value::Null, "id"),
            ),
            ("item".into(), Value::Null),
            ("maxColumnWidth".into(), Value::Null),
            ("isSelected".into(), Value::Null),
        ]
        .into_iter()
        .collect(),
        children: vec![],
    }
}
