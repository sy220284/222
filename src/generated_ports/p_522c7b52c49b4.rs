//! Rust exports derived from original: src/tools.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;
pub fn TOOL_PRESETS() -> Value {
    serde_json::from_str(r#"["default"]"#).expect("Valid source constant")
}
