//! Rust exports derived from original: src/Tool.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

pub fn toolMatchesName(args: &[Value]) -> Value {
    super::runtime::or(
        &super::runtime::strict_equal(
            &super::runtime::property(&args.first().cloned().unwrap_or(Value::Null), "name"),
            &args.get(1).cloned().unwrap_or(Value::Null),
        ),
        &super::runtime::nullish(
            &super::runtime::call_method(
                &super::runtime::property(&args.first().cloned().unwrap_or(Value::Null), "aliases"),
                "includes",
                &[args.get(1).cloned().unwrap_or(Value::Null)],
            ),
            &Value::Bool(false),
        ),
    )
}
