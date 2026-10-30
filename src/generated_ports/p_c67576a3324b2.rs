//! Rust exports derived from original: src/utils/forkedAgent.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::{json, Value};

pub fn createCacheSafeParams(args: &[Value]) -> Value {
    {
        let mut map = serde_json::Map::new();
        map.insert(
            "systemPrompt".into(),
            super::runtime::property(
                &args.first().cloned().unwrap_or(Value::Null),
                "systemPrompt",
            ),
        );
        map.insert(
            "userContext".into(),
            super::runtime::property(&args.first().cloned().unwrap_or(Value::Null), "userContext"),
        );
        map.insert(
            "systemContext".into(),
            super::runtime::property(
                &args.first().cloned().unwrap_or(Value::Null),
                "systemContext",
            ),
        );
        map.insert(
            "toolUseContext".into(),
            super::runtime::property(
                &args.first().cloned().unwrap_or(Value::Null),
                "toolUseContext",
            ),
        );
        map.insert(
            "forkContextMessages".into(),
            super::runtime::property(&args.first().cloned().unwrap_or(Value::Null), "messages"),
        );
        Value::Object(map)
    }
}
