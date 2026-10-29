//! Rust exports derived from original: src/utils/file.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub fn FILE_NOT_FOUND_CWD_NOTE() -> Value {
    serde_json::from_str(r#""Note: your current working directory is""#)
        .expect("Valid source constant")
}
