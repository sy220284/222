//! Rust exports derived from original: src/utils/sessionStorage.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::{json, Value};

pub fn isTranscriptMessage(args: &[Value]) -> Value {
    super::runtime::or(
        &super::runtime::or(
            &super::runtime::or(
                &super::runtime::strict_equal(
                    &super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null), "type"),
                    &Value::String("user".into()),
                ),
                &super::runtime::strict_equal(
                    &super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null), "type"),
                    &Value::String("assistant".into()),
                ),
            ),
            &super::runtime::strict_equal(
                &super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null), "type"),
                &Value::String("attachment".into()),
            ),
        ),
        &super::runtime::strict_equal(
            &super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null), "type"),
            &Value::String("system".into()),
        ),
    )
}
pub fn isChainParticipant(args: &[Value]) -> Value {
    super::runtime::strict_unequal(
        &super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null), "type"),
        &Value::String("progress".into()),
    )
}
pub fn isCustomTitleEnabled(args: &[Value]) -> Value {
    Value::Bool(true)
}
