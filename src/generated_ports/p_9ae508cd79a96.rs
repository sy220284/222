//! Rust exports derived from original: src/constants/product.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::{json, Value};

pub fn isRemoteSessionStaging(args: &[Value]) -> Value {
    super::runtime::or(
        &super::runtime::strict_equal(
            &super::runtime::call_method(
                &args.first().cloned().unwrap_or(Value::Null),
                "includes",
                &[Value::String("_staging_".into())],
            ),
            &Value::Bool(true),
        ),
        &super::runtime::strict_equal(
            &super::runtime::call_method(
                &args.get(1).cloned().unwrap_or(Value::Null),
                "includes",
                &[Value::String("staging".into())],
            ),
            &Value::Bool(true),
        ),
    )
}
pub fn isRemoteSessionLocal(args: &[Value]) -> Value {
    super::runtime::or(
        &super::runtime::strict_equal(
            &super::runtime::call_method(
                &args.first().cloned().unwrap_or(Value::Null),
                "includes",
                &[Value::String("_local_".into())],
            ),
            &Value::Bool(true),
        ),
        &super::runtime::strict_equal(
            &super::runtime::call_method(
                &args.get(1).cloned().unwrap_or(Value::Null),
                "includes",
                &[Value::String("localhost".into())],
            ),
            &Value::Bool(true),
        ),
    )
}
