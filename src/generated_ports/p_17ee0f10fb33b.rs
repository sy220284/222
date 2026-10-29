//! Rust exports derived from original: src/hooks/useTeleportResume.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
pub enum TeleportSource{
    #[serde(rename="cliArg")]
    cliArg,
    #[serde(rename="localCommand")]
    localCommand,
}
