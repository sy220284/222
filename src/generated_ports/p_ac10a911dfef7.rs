//! Rust exports derived from original: src/utils/context.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;
pub fn MODEL_CONTEXT_WINDOW_DEFAULT() -> Value {
    serde_json::from_str(r#"200000"#).expect("Valid source constant")
}

pub fn COMPACT_MAX_OUTPUT_TOKENS() -> Value {
    serde_json::from_str(r#"20000"#).expect("Valid source constant")
}

pub fn CAPPED_DEFAULT_MAX_TOKENS() -> Value {
    serde_json::from_str(r#"8000"#).expect("Valid source constant")
}

pub fn ESCALATED_MAX_TOKENS() -> Value {
    serde_json::from_str(r#"64000"#).expect("Valid source constant")
}
