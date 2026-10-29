//! Rust exports derived from original: src/vim/motions.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::{json, Value};

pub fn isInclusiveMotion(args: &[Value]) -> Value {
    super::runtime::call_method(
        &Value::String("eE$".into()),
        "includes",
        &[args.get(0).cloned().unwrap_or(Value::Null)],
    )
}
pub fn isLinewiseMotion(args: &[Value]) -> Value {
    super::runtime::or(
        &super::runtime::call_method(
            &Value::String("jkG".into()),
            "includes",
            &[args.get(0).cloned().unwrap_or(Value::Null)],
        ),
        &super::runtime::strict_equal(
            &args.get(0).cloned().unwrap_or(Value::Null),
            &Value::String("gg".into()),
        ),
    )
}
