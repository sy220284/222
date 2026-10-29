//! Rust exports derived from original: src/constants/prompts.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
pub fn SYSTEM_PROMPT_DYNAMIC_BOUNDARY()->Value{serde_json::from_str(r#""__SYSTEM_PROMPT_DYNAMIC_BOUNDARY__""#).expect("Valid source constant")}
