//! Rust exports derived from original: src/utils/unaryLogging.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CompletionType {
    #[serde(rename = "str_replace_single")]
    str_replace_single,
    #[serde(rename = "str_replace_multi")]
    str_replace_multi,
    #[serde(rename = "write_file_single")]
    write_file_single,
    #[serde(rename = "tool_use_single")]
    tool_use_single,
}
