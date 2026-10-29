//! Rust exports derived from original: src/utils/plugins/pluginOptionsStorage.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::{json, Value};

pub fn getPluginStorageId(args: &[Value]) -> Value {
    super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null), "source")
}
