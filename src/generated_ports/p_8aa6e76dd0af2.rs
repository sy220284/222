//! Rust exports derived from original: src/utils/toolResultStorage.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub fn TOOL_RESULTS_SUBDIR() -> Value {
    serde_json::from_str(r#""tool-results""#).expect("Valid source constant")
}

pub fn PERSISTED_OUTPUT_TAG() -> Value {
    serde_json::from_str(r#""<persisted-output>""#).expect("Valid source constant")
}

pub fn PERSISTED_OUTPUT_CLOSING_TAG() -> Value {
    serde_json::from_str(r#""</persisted-output>""#).expect("Valid source constant")
}

pub fn TOOL_RESULT_CLEARED_MESSAGE() -> Value {
    serde_json::from_str(r#""[Old tool result content cleared]""#).expect("Valid source constant")
}

pub fn PREVIEW_SIZE_BYTES() -> Value {
    serde_json::from_str(r#"2000"#).expect("Valid source constant")
}
