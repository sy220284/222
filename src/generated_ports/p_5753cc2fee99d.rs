//! Rust exports derived from original: src/ink/termio/dec.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub fn DEC() -> Value {
    serde_json::from_str(r#"{"CURSOR_VISIBLE":25,"ALT_SCREEN":47,"ALT_SCREEN_CLEAR":1049,"MOUSE_NORMAL":1000,"MOUSE_BUTTON":1002,"MOUSE_ANY":1003,"MOUSE_SGR":1006,"FOCUS_EVENTS":1004,"BRACKETED_PASTE":2004,"SYNCHRONIZED_UPDATE":2026}"#).expect("Valid source constant")
}
