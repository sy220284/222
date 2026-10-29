//! Rust exports derived from original: src/ink/layout/geometry.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub fn ZERO_EDGES() -> Value {
    serde_json::from_str(r#"{"top":0,"right":0,"bottom":0,"left":0}"#)
        .expect("Valid source constant")
}

use serde_json::json;

pub fn addEdges(args: &[Value]) -> Value {
    {
        let mut map = serde_json::Map::new();
        map.insert(
            "top".into(),
            super::runtime::add(
                &super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null), "top"),
                &super::runtime::property(&args.get(1).cloned().unwrap_or(Value::Null), "top"),
            ),
        );
        map.insert(
            "right".into(),
            super::runtime::add(
                &super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null), "right"),
                &super::runtime::property(&args.get(1).cloned().unwrap_or(Value::Null), "right"),
            ),
        );
        map.insert(
            "bottom".into(),
            super::runtime::add(
                &super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null), "bottom"),
                &super::runtime::property(&args.get(1).cloned().unwrap_or(Value::Null), "bottom"),
            ),
        );
        map.insert(
            "left".into(),
            super::runtime::add(
                &super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null), "left"),
                &super::runtime::property(&args.get(1).cloned().unwrap_or(Value::Null), "left"),
            ),
        );
        Value::Object(map)
    }
}
pub fn resolveEdges(args: &[Value]) -> Value {
    {
        let mut map = serde_json::Map::new();
        map.insert(
            "top".into(),
            super::runtime::nullish(
                &super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null), "top"),
                &json!(0),
            ),
        );
        map.insert(
            "right".into(),
            super::runtime::nullish(
                &super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null), "right"),
                &json!(0),
            ),
        );
        map.insert(
            "bottom".into(),
            super::runtime::nullish(
                &super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null), "bottom"),
                &json!(0),
            ),
        );
        map.insert(
            "left".into(),
            super::runtime::nullish(
                &super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null), "left"),
                &json!(0),
            ),
        );
        Value::Object(map)
    }
}
pub fn withinBounds(args: &[Value]) -> Value {
    super::runtime::and(
        &super::runtime::and(
            &super::runtime::and(
                &super::runtime::gte(
                    &super::runtime::property(&args.get(1).cloned().unwrap_or(Value::Null), "x"),
                    &json!(0),
                ),
                &super::runtime::gte(
                    &super::runtime::property(&args.get(1).cloned().unwrap_or(Value::Null), "y"),
                    &json!(0),
                ),
            ),
            &super::runtime::lt(
                &super::runtime::property(&args.get(1).cloned().unwrap_or(Value::Null), "x"),
                &super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null), "width"),
            ),
        ),
        &super::runtime::lt(
            &super::runtime::property(&args.get(1).cloned().unwrap_or(Value::Null), "y"),
            &super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null), "height"),
        ),
    )
}
