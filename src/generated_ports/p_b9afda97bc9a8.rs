//! Rust exports derived from original: src/utils/statsCache.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;
pub fn STATS_CACHE_VERSION() -> Value {
    serde_json::from_str(r#"3"#).expect("Valid source constant")
}

pub fn isDateBefore(args: &[Value]) -> Value {
    super::runtime::lt(
        &args.first().cloned().unwrap_or(Value::Null),
        &args.get(1).cloned().unwrap_or(Value::Null),
    )
}
