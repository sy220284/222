//! Rust exports derived from original: src/ink/terminal-focus-state.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
pub enum TerminalFocusState{
    #[serde(rename="focused")]
    focused,
    #[serde(rename="blurred")]
    blurred,
    #[serde(rename="unknown")]
    unknown,
}
