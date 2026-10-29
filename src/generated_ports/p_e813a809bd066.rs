//! Rust exports derived from original: src/utils/plugins/schemas.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::{json, Value};

pub fn isLocalMarketplaceSource(args: &[Value]) -> Value {
    super::runtime::or(
        &super::runtime::strict_equal(
            &super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null), "source"),
            &Value::String("file".into()),
        ),
        &super::runtime::strict_equal(
            &super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null), "source"),
            &Value::String("directory".into()),
        ),
    )
}
