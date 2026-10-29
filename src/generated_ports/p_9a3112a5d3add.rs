//! Rust exports derived from original: src/utils/fastMode.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub fn FAST_MODE_MODEL_DISPLAY() -> Value {
    serde_json::from_str(r#""Opus 4.6""#).expect("Valid source constant")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CooldownReason {
    #[serde(rename = "rate_limit")]
    rate_limit,
    #[serde(rename = "overloaded")]
    overloaded,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FastModeDisabledReason {
    #[serde(rename = "free")]
    free,
    #[serde(rename = "preference")]
    preference,
    #[serde(rename = "extra_usage_disabled")]
    extra_usage_disabled,
    #[serde(rename = "network_error")]
    network_error,
    #[serde(rename = "unknown")]
    unknown,
}
