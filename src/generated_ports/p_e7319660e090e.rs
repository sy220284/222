//! Rust exports derived from original: src/utils/api.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
pub enum CacheScope{
    #[serde(rename="global")]
    global,
    #[serde(rename="org")]
    org,
}
