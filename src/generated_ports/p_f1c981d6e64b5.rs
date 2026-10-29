//! Rust exports derived from original: src/ink/frame.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FlickerReason {
    #[serde(rename = "resize")]
    resize,
    #[serde(rename = "offscreen")]
    offscreen,
    #[serde(rename = "clear")]
    clear,
}
