//! Rust exports derived from original: src/utils/contextSuggestions.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
pub enum SuggestionSeverity{
    #[serde(rename="info")]
    info,
    #[serde(rename="warning")]
    warning,
}
