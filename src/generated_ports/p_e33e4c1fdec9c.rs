//! Rust exports derived from original: src/utils/bash/shellCompletion.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
pub enum ShellCompletionType{
    #[serde(rename="command")]
    command,
    #[serde(rename="variable")]
    variable,
    #[serde(rename="file")]
    file,
}
