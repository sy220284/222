//! Rust exports derived from original: src/utils/stats.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
pub enum StatsDateRange{
    #[serde(rename="7d")]
    n_7d,
    #[serde(rename="30d")]
    n_30d,
    #[serde(rename="all")]
    all,
}
