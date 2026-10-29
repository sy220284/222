//! Rust exports derived from original: src/cli/transports/ccrClient.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
pub enum CCRInitFailReason{
    #[serde(rename="no_auth_headers")]
    no_auth_headers,
    #[serde(rename="missing_epoch")]
    missing_epoch,
    #[serde(rename="worker_register_failed")]
    worker_register_failed,
}
