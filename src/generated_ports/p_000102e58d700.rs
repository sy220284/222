//! Rust exports derived from original: src/utils/sessionState.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SessionState {
    #[serde(rename = "idle")]
    idle,
    #[serde(rename = "running")]
    running,
    #[serde(rename = "requires_action")]
    requires_action,
}
