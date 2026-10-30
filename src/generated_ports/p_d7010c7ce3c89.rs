//! Rust exports derived from original: src/utils/doctorDiagnostic.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum InstallationType {
    #[serde(rename = "npm-global")]
    npm_global,
    #[serde(rename = "npm-local")]
    npm_local,
    #[serde(rename = "native")]
    native,
    #[serde(rename = "package-manager")]
    package_manager,
    #[serde(rename = "development")]
    development,
    #[serde(rename = "unknown")]
    unknown,
}
