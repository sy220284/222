//! Rust exports derived from original: src/types/generated/events_mono/common/v1/auth.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicApiAuth {
    #[serde(rename = "account_id", default)]
    pub account_id: Option<Value>,
    #[serde(rename = "organization_uuid", default)]
    pub organization_uuid: Option<Value>,
    #[serde(rename = "account_uuid", default)]
    pub account_uuid: Option<Value>,
}
