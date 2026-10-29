//! Rust exports derived from original: src/utils/autoUpdater.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum InstallStatus {
    #[serde(rename = "success")]
    success,
    #[serde(rename = "no_permissions")]
    no_permissions,
    #[serde(rename = "install_failed")]
    install_failed,
    #[serde(rename = "in_progress")]
    in_progress,
}
