//! Rust exports derived from original: src/hooks/useNotifyAfterTimeout.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;
pub fn DEFAULT_INTERACTION_THRESHOLD_MS() -> Value {
    serde_json::from_str(r#"6000"#).expect("Valid source constant")
}
