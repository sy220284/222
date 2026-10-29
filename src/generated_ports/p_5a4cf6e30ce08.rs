//! Rust exports derived from original: src/hooks/useVoice.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub fn FIRST_PRESS_FALLBACK_MS() -> Value {
    serde_json::from_str(r#"2000"#).expect("Valid source constant")
}
