//! Rust exports derived from original: src/components/grove/Grove.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
pub enum GroveDecision{
    #[serde(rename="accept_opt_in")]
    accept_opt_in,
    #[serde(rename="accept_opt_out")]
    accept_opt_out,
    #[serde(rename="defer")]
    defer,
    #[serde(rename="escape")]
    escape,
    #[serde(rename="skip_rendering")]
    skip_rendering,
}
