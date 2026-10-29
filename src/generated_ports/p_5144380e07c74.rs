//! Rust exports derived from original: src/types/generated/google/protobuf/timestamp.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct Timestamp{
    #[serde(rename="seconds",default)]
    pub seconds:Option<Value>,
    #[serde(rename="nanos",default)]
    pub nanos:Option<Value>,
}
