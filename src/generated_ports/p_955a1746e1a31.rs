//! Rust exports derived from original: src/utils/suggestions/commandSuggestions.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::{json, Value};

pub fn isCommandInput(args: &[Value]) -> Value {
    super::runtime::call_method(
        &args.get(0).cloned().unwrap_or(Value::Null),
        "startsWith",
        &[Value::String("/".into())],
    )
}
pub fn formatCommand(args: &[Value]) -> Value {
    super::runtime::concat(&[
        Value::String("/".into()),
        args.get(0).cloned().unwrap_or(Value::Null),
        Value::String(" ".into()),
    ])
}
