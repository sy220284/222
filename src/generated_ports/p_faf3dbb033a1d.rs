//! Rust exports derived from original: src/utils/tokens.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::{json, Value};

pub fn getTokenCountFromUsage(args: &[Value]) -> Value {
    super::runtime::add(
        &super::runtime::add(
            &super::runtime::add(
                &super::runtime::property(
                    &args.get(0).cloned().unwrap_or(Value::Null),
                    "input_tokens",
                ),
                &super::runtime::nullish(
                    &super::runtime::property(
                        &args.get(0).cloned().unwrap_or(Value::Null),
                        "cache_creation_input_tokens",
                    ),
                    &json!(0),
                ),
            ),
            &super::runtime::nullish(
                &super::runtime::property(
                    &args.get(0).cloned().unwrap_or(Value::Null),
                    "cache_read_input_tokens",
                ),
                &json!(0),
            ),
        ),
        &super::runtime::property(
            &args.get(0).cloned().unwrap_or(Value::Null),
            "output_tokens",
        ),
    )
}
