//! Rust exports derived from original: src/hooks/useDoublePress.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub fn DOUBLE_PRESS_TIMEOUT_MS() -> Value {
    serde_json::from_str(r#"800"#).expect("Valid source constant")
}
