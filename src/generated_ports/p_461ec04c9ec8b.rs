//! Rust exports derived from original: src/types/generated/events_mono/growthbook/v1/growthbook_experiment_event.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrowthbookExperimentEvent {
    #[serde(rename = "event_id", default)]
    pub event_id: Option<Value>,
    #[serde(rename = "timestamp", default)]
    pub timestamp: Option<Value>,
    #[serde(rename = "experiment_id", default)]
    pub experiment_id: Option<Value>,
    #[serde(rename = "variation_id", default)]
    pub variation_id: Option<Value>,
    #[serde(rename = "environment", default)]
    pub environment: Option<Value>,
    #[serde(rename = "user_attributes", default)]
    pub user_attributes: Option<Value>,
    #[serde(rename = "experiment_metadata", default)]
    pub experiment_metadata: Option<Value>,
    #[serde(rename = "device_id", default)]
    pub device_id: Option<Value>,
    #[serde(rename = "auth", default)]
    pub auth: Option<Value>,
    #[serde(rename = "session_id", default)]
    pub session_id: Option<Value>,
    #[serde(rename = "anonymous_id", default)]
    pub anonymous_id: Option<Value>,
    #[serde(rename = "event_metadata_vars", default)]
    pub event_metadata_vars: Option<Value>,
}
