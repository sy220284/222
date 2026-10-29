//! Rust exports derived from original: src/components/mcp/utils/reconnectHelpers.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct ReconnectResult{
    #[serde(rename="message")]
    pub message:String,
    #[serde(rename="success")]
    pub success:bool,
}
