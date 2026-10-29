//! Rust exports derived from original: src/ink/events/event-handlers.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
pub fn HANDLER_FOR_EVENT()->Value{serde_json::from_str(r#"{"keydown":{"bubble":"onKeyDown","capture":"onKeyDownCapture"},"focus":{"bubble":"onFocus","capture":"onFocusCapture"},"blur":{"bubble":"onBlur","capture":"onBlurCapture"},"paste":{"bubble":"onPaste","capture":"onPasteCapture"},"resize":{"bubble":"onResize"},"click":{"bubble":"onClick"}}"#).expect("Valid source constant")}
