//! Rust exports derived from original: src/utils/statusNoticeDefinitions.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum StatusNoticeType {
    #[serde(rename = "warning")]
    warning,
    #[serde(rename = "info")]
    info,
}
