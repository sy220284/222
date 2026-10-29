//! Rust exports derived from original: src/utils/fileRead.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LineEndingType {
    #[serde(rename = "CRLF")]
    CRLF,
    #[serde(rename = "LF")]
    LF,
}
