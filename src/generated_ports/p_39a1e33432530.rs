//! Rust exports derived from original: src/utils/streamJsonStdoutGuard.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub fn STDOUT_GUARD_MARKER() -> Value {
    serde_json::from_str(r#""[stdout-guard]""#).expect("Valid source constant")
}
