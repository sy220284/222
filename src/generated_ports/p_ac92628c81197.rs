//! Rust exports derived from original: src/services/PromptSuggestion/promptSuggestion.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PromptVariant {
    #[serde(rename = "user_intent")]
    user_intent,
    #[serde(rename = "stated_intent")]
    stated_intent,
}

pub fn getPromptVariant(args: &[Value]) -> Value {
    Value::String("user_intent".into())
}
