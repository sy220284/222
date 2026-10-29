//! Rust exports derived from original: src/services/plugins/pluginOperations.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;
pub fn VALID_INSTALLABLE_SCOPES() -> Value {
    serde_json::from_str(r#"["user","project","local"]"#).expect("Valid source constant")
}

pub fn VALID_UPDATE_SCOPES() -> Value {
    serde_json::from_str(r#"["user","project","local","managed"]"#).expect("Valid source constant")
}
