//! Rust exports derived from original: src/server/types.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SessionState {
    #[serde(rename = "starting")]
    starting,
    #[serde(rename = "running")]
    running,
    #[serde(rename = "detached")]
    detached,
    #[serde(rename = "stopping")]
    stopping,
    #[serde(rename = "stopped")]
    stopped,
}
