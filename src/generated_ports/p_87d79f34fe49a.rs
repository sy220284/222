//! Rust exports derived from original: src/types/ids.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::{Value,json};

pub fn asSessionId(args:&[Value])->Value {args.get(0).cloned().unwrap_or(Value::Null)}
pub fn asAgentId(args:&[Value])->Value {args.get(0).cloned().unwrap_or(Value::Null)}
