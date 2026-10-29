//! Rust exports derived from original: src/utils/toolSearch.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ToolSearchMode {
    #[serde(rename = "tst")]
    tst,
    #[serde(rename = "tst-auto")]
    tst_auto,
    #[serde(rename = "standard")]
    standard,
}
