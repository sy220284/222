//! Rust exports derived from original: src/utils/ghPrStatus.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PrReviewState {
    #[serde(rename = "approved")]
    approved,
    #[serde(rename = "pending")]
    pending,
    #[serde(rename = "changes_requested")]
    changes_requested,
    #[serde(rename = "draft")]
    draft,
    #[serde(rename = "merged")]
    merged,
    #[serde(rename = "closed")]
    closed,
}
