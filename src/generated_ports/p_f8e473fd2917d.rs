//! Rust exports derived from original: src/keybindings/validate.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum KeybindingWarningType {
    #[serde(rename = "parse_error")]
    parse_error,
    #[serde(rename = "duplicate")]
    duplicate,
    #[serde(rename = "reserved")]
    reserved,
    #[serde(rename = "invalid_context")]
    invalid_context,
    #[serde(rename = "invalid_action")]
    invalid_action,
}
