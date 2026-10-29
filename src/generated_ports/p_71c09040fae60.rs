//! Rust exports derived from original: src/utils/settings/types.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
pub fn CUSTOMIZATION_SURFACES()->Value{serde_json::from_str(r#"["skills","agents","hooks","mcp"]"#).expect("Valid source constant")}
