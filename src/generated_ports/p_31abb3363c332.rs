//! Rust exports derived from original: src/utils/sessionActivity.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SessionActivityReason {
    #[serde(rename = "api_call")]
    api_call,
    #[serde(rename = "tool_exec")]
    tool_exec,
}
