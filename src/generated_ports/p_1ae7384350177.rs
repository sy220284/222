//! Rust exports derived from original: src/utils/teleport/api.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
pub fn CCR_BYOC_BETA()->Value{serde_json::from_str(r#""ccr-byoc-2025-07-29""#).expect("Valid source constant")}

#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
pub enum SessionStatus{
    #[serde(rename="requires_action")]
    requires_action,
    #[serde(rename="running")]
    running,
    #[serde(rename="idle")]
    idle,
    #[serde(rename="archived")]
    archived,
}
