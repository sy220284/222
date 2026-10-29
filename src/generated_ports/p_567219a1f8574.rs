//! Rust exports derived from original: src/utils/preflightChecks.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflightCheckResult {
    #[serde(rename = "success")]
    pub success: bool,
    #[serde(rename = "error", default)]
    pub error: Option<String>,
    #[serde(rename = "sslHint", default)]
    pub sslHint: Option<String>,
}
