//! Rust exports derived from original: src/hooks/useMemoryUsage.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
pub enum MemoryUsageStatus{
    #[serde(rename="normal")]
    normal,
    #[serde(rename="high")]
    high,
    #[serde(rename="critical")]
    critical,
}
