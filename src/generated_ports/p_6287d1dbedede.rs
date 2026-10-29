//! Rust exports derived from original: src/utils/mcpValidation.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
pub fn MCP_TOKEN_COUNT_THRESHOLD_FACTOR()->Value{serde_json::from_str(r#"0.5"#).expect("Valid source constant")}

pub fn IMAGE_TOKEN_ESTIMATE()->Value{serde_json::from_str(r#"1600"#).expect("Valid source constant")}
