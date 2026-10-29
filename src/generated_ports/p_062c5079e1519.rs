//! Rust exports derived from original: src/utils/effort.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
pub fn EFFORT_LEVELS()->Value{serde_json::from_str(r#"["low","medium","high","max"]"#).expect("Valid source constant")}
