//! Rust exports derived from original: src/types/textInputTypes.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum VimMode {
    #[serde(rename = "INSERT")]
    INSERT,
    #[serde(rename = "NORMAL")]
    NORMAL,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PromptInputMode {
    #[serde(rename = "bash")]
    bash,
    #[serde(rename = "prompt")]
    prompt,
    #[serde(rename = "orphaned-permission")]
    orphaned_permission,
    #[serde(rename = "task-notification")]
    task_notification,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum QueuePriority {
    #[serde(rename = "now")]
    now,
    #[serde(rename = "next")]
    next,
    #[serde(rename = "later")]
    later,
}

use serde_json::json;

pub fn isValidImagePaste(args: &[Value]) -> Value {
    super::runtime::and(
        &super::runtime::strict_equal(
            &super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null), "type"),
            &Value::String("image".into()),
        ),
        &super::runtime::gt(
            &super::runtime::property(
                &super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null), "content"),
                "length",
            ),
            &json!(0),
        ),
    )
}
