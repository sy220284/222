//! Rust exports derived from original: src/utils/ansiToSvg.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub fn DEFAULT_FG() -> Value {
    serde_json::from_str(r#"{"r":229,"g":229,"b":229}"#).expect("Valid source constant")
}

pub fn DEFAULT_BG() -> Value {
    serde_json::from_str(r#"{"r":30,"g":30,"b":30}"#).expect("Valid source constant")
}
