//! Rust exports derived from original: src/utils/systemTheme.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SystemTheme {
    #[serde(rename = "dark")]
    dark,
    #[serde(rename = "light")]
    light,
}
