//! Rust exports derived from original: src/ink/dom.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ElementNames {
    #[serde(rename = "ink-root")]
    ink_root,
    #[serde(rename = "ink-box")]
    ink_box,
    #[serde(rename = "ink-text")]
    ink_text,
    #[serde(rename = "ink-virtual-text")]
    ink_virtual_text,
    #[serde(rename = "ink-link")]
    ink_link,
    #[serde(rename = "ink-progress")]
    ink_progress,
    #[serde(rename = "ink-raw-ansi")]
    ink_raw_ansi,
}
