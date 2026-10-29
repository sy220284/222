//! Rust exports derived from original: src/ink/parse-keypress.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub fn DECRPM_STATUS() -> Value {
    serde_json::from_str(
        r#"{"NOT_RECOGNIZED":0,"SET":1,"RESET":2,"PERMANENTLY_SET":3,"PERMANENTLY_RESET":4}"#,
    )
    .expect("Valid source constant")
}

pub fn INITIAL_STATE() -> Value {
    serde_json::from_str(r#"{"mode":"NORMAL","incomplete":"","pasteBuffer":""}"#)
        .expect("Valid source constant")
}
