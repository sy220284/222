//! Rust exports derived from original: src/utils/memoryFileDetection.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MemoryScope {
    #[serde(rename = "personal")]
    personal,
    #[serde(rename = "team")]
    team,
}
