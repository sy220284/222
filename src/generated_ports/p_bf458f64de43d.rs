//! Rust exports derived from original: src/utils/errors.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
pub enum AxiosErrorKind{
    #[serde(rename="auth")]
    auth,
    #[serde(rename="timeout")]
    timeout,
    #[serde(rename="network")]
    network,
    #[serde(rename="http")]
    http,
    #[serde(rename="other")]
    other,
}
