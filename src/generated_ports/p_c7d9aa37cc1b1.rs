//! Rust exports derived from original: src/keybindings/resolver.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::{json, Value};

pub fn keystrokesEqual(args: &[Value]) -> Value {
    super::runtime::and(
        &super::runtime::and(
            &super::runtime::and(
                &super::runtime::and(
                    &super::runtime::strict_equal(
                        &super::runtime::property(
                            &args.first().cloned().unwrap_or(Value::Null),
                            "key",
                        ),
                        &super::runtime::property(
                            &args.get(1).cloned().unwrap_or(Value::Null),
                            "key",
                        ),
                    ),
                    &super::runtime::strict_equal(
                        &super::runtime::property(
                            &args.first().cloned().unwrap_or(Value::Null),
                            "ctrl",
                        ),
                        &super::runtime::property(
                            &args.get(1).cloned().unwrap_or(Value::Null),
                            "ctrl",
                        ),
                    ),
                ),
                &super::runtime::strict_equal(
                    &super::runtime::property(
                        &args.first().cloned().unwrap_or(Value::Null),
                        "shift",
                    ),
                    &super::runtime::property(
                        &args.get(1).cloned().unwrap_or(Value::Null),
                        "shift",
                    ),
                ),
            ),
            &super::runtime::strict_equal(
                &super::runtime::or(
                    &super::runtime::property(&args.first().cloned().unwrap_or(Value::Null), "alt"),
                    &super::runtime::property(
                        &args.first().cloned().unwrap_or(Value::Null),
                        "meta",
                    ),
                ),
                &super::runtime::or(
                    &super::runtime::property(&args.get(1).cloned().unwrap_or(Value::Null), "alt"),
                    &super::runtime::property(&args.get(1).cloned().unwrap_or(Value::Null), "meta"),
                ),
            ),
        ),
        &super::runtime::strict_equal(
            &super::runtime::property(&args.first().cloned().unwrap_or(Value::Null), "super"),
            &super::runtime::property(&args.get(1).cloned().unwrap_or(Value::Null), "super"),
        ),
    )
}
