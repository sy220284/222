//! Rust exports derived from original: src/components/Spinner/utils.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::{json, Value};

pub fn toRGBColor(args: &[Value]) -> Value {
    super::runtime::concat(&[
        Value::String("rgb(".into()),
        super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null), "r"),
        Value::String(",".into()),
        super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null), "g"),
        Value::String(",".into()),
        super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null), "b"),
        Value::String(")".into()),
    ])
}
