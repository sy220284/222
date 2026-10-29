//! Rust exports derived from original: src/utils/nativeInstaller/installer.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
pub fn VERSION_RETENTION_COUNT()->Value{serde_json::from_str(r#"2"#).expect("Valid source constant")}
