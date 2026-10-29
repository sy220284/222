//! Rust exports derived from original: src/ink/termio/osc.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ClipboardPath {
    #[serde(rename = "native")]
    native,
    #[serde(rename = "tmux-buffer")]
    tmux_buffer,
    #[serde(rename = "osc52")]
    osc52,
}

pub fn OSC() -> Value {
    serde_json::from_str(r#"{"SET_TITLE_AND_ICON":0,"SET_ICON":1,"SET_TITLE":2,"SET_COLOR":4,"SET_CWD":7,"HYPERLINK":8,"ITERM2":9,"SET_FG_COLOR":10,"SET_BG_COLOR":11,"SET_CURSOR_COLOR":12,"CLIPBOARD":52,"KITTY":99,"RESET_COLOR":104,"RESET_FG_COLOR":110,"RESET_BG_COLOR":111,"RESET_CURSOR_COLOR":112,"SEMANTIC_PROMPT":133,"GHOSTTY":777,"TAB_STATUS":21337}"#).expect("Valid source constant")
}

pub fn ITERM2() -> Value {
    serde_json::from_str(r#"{"NOTIFY":0,"BADGE":2,"PROGRESS":4}"#).expect("Valid source constant")
}

pub fn PROGRESS() -> Value {
    serde_json::from_str(r#"{"CLEAR":0,"SET":1,"ERROR":2,"INDETERMINATE":3}"#)
        .expect("Valid source constant")
}
