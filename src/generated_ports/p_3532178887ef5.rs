//! Rust exports derived from original: src/assistant/sessionHistory.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub fn HISTORY_PAGE_SIZE() -> Value {
    serde_json::from_str(r#"100"#).expect("Valid source constant")
}
