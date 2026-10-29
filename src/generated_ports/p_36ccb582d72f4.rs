//! Rust exports derived from original: src/ink/colorize.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ColorType {
    #[serde(rename = "foreground")]
    foreground,
    #[serde(rename = "background")]
    background,
}
