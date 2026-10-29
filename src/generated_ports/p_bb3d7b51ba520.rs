//! Rust exports derived from original: src/utils/plugins/zipCache.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::{json, Value};

pub fn isMarketplaceSourceSupportedByZipCache(args: &[Value]) -> Value {
    super::runtime::call_method(
        &Value::Array(vec![
            Value::String("github".into()),
            Value::String("git".into()),
            Value::String("url".into()),
            Value::String("settings".into()),
        ]),
        "includes",
        &[super::runtime::property(
            &args.first().cloned().unwrap_or(Value::Null),
            "source",
        )],
    )
}
