//! Rust exports derived from original: src/utils/bash/ParsedCommand.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IParsedCommand {
    #[serde(rename = "originalCommand")]
    pub originalCommand: String,
}
