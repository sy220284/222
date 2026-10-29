//! Rust exports derived from original: src/utils/settings/constants.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub fn SETTING_SOURCES() -> Value {
    serde_json::from_str(
        r#"["userSettings","projectSettings","localSettings","flagSettings","policySettings"]"#,
    )
    .expect("Valid source constant")
}

pub fn SOURCES() -> Value {
    serde_json::from_str(r#"["localSettings","projectSettings","userSettings"]"#)
        .expect("Valid source constant")
}
