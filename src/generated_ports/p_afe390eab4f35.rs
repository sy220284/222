//! Rust exports derived from original: src/utils/config.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedStructuredHistoryEntry {
    #[serde(rename = "display")]
    pub display: String,
    #[serde(rename = "pastedContents", default)]
    pub pastedContents: Option<Value>,
    #[serde(rename = "pastedText", default)]
    pub pastedText: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    #[serde(rename = "display")]
    pub display: String,
    #[serde(rename = "pastedContents")]
    pub pastedContents: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReleaseChannel {
    #[serde(rename = "stable")]
    stable,
    #[serde(rename = "latest")]
    latest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum InstallMethod {
    #[serde(rename = "local")]
    local,
    #[serde(rename = "native")]
    native,
    #[serde(rename = "global")]
    global,
    #[serde(rename = "unknown")]
    unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DiffTool {
    #[serde(rename = "terminal")]
    terminal,
    #[serde(rename = "auto")]
    auto_,
}

pub fn PROJECT_CONFIG_KEYS() -> Value {
    serde_json::from_str(
        r#"["allowedTools","hasTrustDialogAccepted","hasCompletedProjectOnboarding"]"#,
    )
    .expect("Valid source constant")
}

pub fn CONFIG_WRITE_DISPLAY_THRESHOLD() -> Value {
    serde_json::from_str(r#"20"#).expect("Valid source constant")
}
