//! Rust exports derived from original: src/ink/hooks/use-tab-status.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TabStatusKind {
    #[serde(rename = "idle")]
    idle,
    #[serde(rename = "busy")]
    busy,
    #[serde(rename = "waiting")]
    waiting,
}
