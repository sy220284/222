//! Rust exports derived from original: src/utils/imagePaste.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
pub fn PASTE_THRESHOLD()->Value{serde_json::from_str(r#"800"#).expect("Valid source constant")}
