//! Rust exports derived from original: src/state/AppStateStore.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub fn IDLE_SPECULATION_STATE() -> Value {
    serde_json::from_str(r#"{"status":"idle"}"#).expect("Valid source constant")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FooterItem {
    #[serde(rename = "tasks")]
    tasks,
    #[serde(rename = "tmux")]
    tmux,
    #[serde(rename = "bagel")]
    bagel,
    #[serde(rename = "teams")]
    teams,
    #[serde(rename = "bridge")]
    bridge,
    #[serde(rename = "companion")]
    companion,
}
