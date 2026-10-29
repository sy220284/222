//! Rust exports derived from original: src/utils/settings/mdm/constants.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub fn WINDOWS_REGISTRY_VALUE_NAME() -> Value {
    serde_json::from_str(r#""Settings""#).expect("Valid source constant")
}

pub fn PLUTIL_PATH() -> Value {
    serde_json::from_str(r#""/usr/bin/plutil""#).expect("Valid source constant")
}

pub fn PLUTIL_ARGS_PREFIX() -> Value {
    serde_json::from_str(r#"["-convert","json","-o","-","--"]"#).expect("Valid source constant")
}

pub fn MDM_SUBPROCESS_TIMEOUT_MS() -> Value {
    serde_json::from_str(r#"5000"#).expect("Valid source constant")
}
