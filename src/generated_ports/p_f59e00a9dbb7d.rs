//! Rust exports derived from original: src/hooks/useApiKeyVerification.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
pub enum VerificationStatus{
    #[serde(rename="loading")]
    loading,
    #[serde(rename="valid")]
    valid,
    #[serde(rename="invalid")]
    invalid,
    #[serde(rename="missing")]
    missing,
    #[serde(rename="error")]
    error,
}
