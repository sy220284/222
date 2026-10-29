//! Rust exports derived from original: src/utils/swarm/backends/teammateModeSnapshot.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
pub enum TeammateMode{
    #[serde(rename="auto")]
    auto_,
    #[serde(rename="tmux")]
    tmux,
    #[serde(rename="in-process")]
    in_process,
}
