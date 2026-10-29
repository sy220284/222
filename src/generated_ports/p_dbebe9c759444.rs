//! Rust exports derived from original: src/utils/platform.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
pub enum Platform{
    #[serde(rename="macos")]
    macos,
    #[serde(rename="windows")]
    windows,
    #[serde(rename="wsl")]
    wsl,
    #[serde(rename="linux")]
    linux,
    #[serde(rename="unknown")]
    unknown,
}

pub fn SUPPORTED_PLATFORMS()->Value{serde_json::from_str(r#"["macos","wsl"]"#).expect("Valid source constant")}
