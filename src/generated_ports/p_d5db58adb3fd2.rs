//! Rust exports derived from original: src/utils/plugins/pluginIdentifier.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub fn SETTING_SOURCE_TO_SCOPE() -> Value {
    serde_json::from_str(r#"{"policySettings":"managed","userSettings":"user","projectSettings":"project","localSettings":"local","flagSettings":"flag"}"#).expect("Valid source constant")
}

use serde_json::json;

pub fn buildPluginId(args: &[Value]) -> Value {
    if super::runtime::truthy(&args.get(1).cloned().unwrap_or(Value::Null)) {
        super::runtime::concat(&[
            Value::String("".into()),
            args.get(0).cloned().unwrap_or(Value::Null),
            Value::String("@".into()),
            args.get(1).cloned().unwrap_or(Value::Null),
            Value::String("".into()),
        ])
    } else {
        args.get(0).cloned().unwrap_or(Value::Null)
    }
}
