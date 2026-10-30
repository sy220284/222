//! Rust exports derived from original: src/types/ids.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::{json, Value};

pub fn asSessionId(args: &[Value]) -> Value {
    args.first().cloned().unwrap_or(Value::Null)
}
pub fn asAgentId(args: &[Value]) -> Value {
    args.first().cloned().unwrap_or(Value::Null)
}
