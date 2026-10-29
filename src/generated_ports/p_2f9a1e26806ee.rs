//! Rust exports derived from original: src/plugins/builtinPlugins.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub fn BUILTIN_MARKETPLACE_NAME() -> Value {
    serde_json::from_str(r#""builtin""#).expect("Valid source constant")
}
