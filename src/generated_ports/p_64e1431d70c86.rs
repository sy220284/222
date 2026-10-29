//! Rust exports derived from original: src/utils/sessionStoragePortable.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub fn LITE_READ_BUF_SIZE() -> Value {
    serde_json::from_str(r#"65536"#).expect("Valid source constant")
}

pub fn MAX_SANITIZED_LENGTH() -> Value {
    serde_json::from_str(r#"200"#).expect("Valid source constant")
}
