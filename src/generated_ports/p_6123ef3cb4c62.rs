//! Rust exports derived from original: src/utils/warningHandler.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub fn MAX_WARNING_KEYS() -> Value {
    serde_json::from_str(r#"1000"#).expect("Valid source constant")
}
