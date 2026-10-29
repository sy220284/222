//! Rust exports derived from original: src/utils/swarm/backends/types.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BackendType {
    #[serde(rename = "tmux")]
    tmux,
    #[serde(rename = "iterm2")]
    iterm2,
    #[serde(rename = "in-process")]
    in_process,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PaneBackendType {
    #[serde(rename = "tmux")]
    tmux,
    #[serde(rename = "iterm2")]
    iterm2,
}

use serde_json::json;

pub fn isPaneBackend(args: &[Value]) -> Value {
    super::runtime::or(
        &super::runtime::strict_equal(
            &args.get(0).cloned().unwrap_or(Value::Null),
            &Value::String("tmux".into()),
        ),
        &super::runtime::strict_equal(
            &args.get(0).cloned().unwrap_or(Value::Null),
            &Value::String("iterm2".into()),
        ),
    )
}
