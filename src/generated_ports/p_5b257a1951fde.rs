//! Rust exports derived from original: src/utils/debug.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
pub enum DebugLogLevel{
    #[serde(rename="verbose")]
    verbose,
    #[serde(rename="debug")]
    debug,
    #[serde(rename="info")]
    info,
    #[serde(rename="warn")]
    warn,
    #[serde(rename="error")]
    error,
}
