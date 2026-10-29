//! Rust exports derived from original: src/utils/concurrentSessions.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SessionKind {
    #[serde(rename = "interactive")]
    interactive,
    #[serde(rename = "bg")]
    bg,
    #[serde(rename = "daemon")]
    daemon,
    #[serde(rename = "daemon-worker")]
    daemon_worker,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SessionStatus {
    #[serde(rename = "busy")]
    busy,
    #[serde(rename = "idle")]
    idle,
    #[serde(rename = "waiting")]
    waiting,
}
