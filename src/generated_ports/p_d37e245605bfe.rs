//! Rust exports derived from original: src/utils/suggestions/directoryCompletion.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::{json, Value};

pub fn isPathLikeToken(args: &[Value]) -> Value {
    super::runtime::or(
        &super::runtime::or(
            &super::runtime::or(
                &super::runtime::or(
                    &super::runtime::or(
                        &super::runtime::or(
                            &super::runtime::call_method(
                                &args.first().cloned().unwrap_or(Value::Null),
                                "startsWith",
                                &[Value::String("~/".into())],
                            ),
                            &super::runtime::call_method(
                                &args.first().cloned().unwrap_or(Value::Null),
                                "startsWith",
                                &[Value::String("/".into())],
                            ),
                        ),
                        &super::runtime::call_method(
                            &args.first().cloned().unwrap_or(Value::Null),
                            "startsWith",
                            &[Value::String("./".into())],
                        ),
                    ),
                    &super::runtime::call_method(
                        &args.first().cloned().unwrap_or(Value::Null),
                        "startsWith",
                        &[Value::String("../".into())],
                    ),
                ),
                &super::runtime::strict_equal(
                    &args.first().cloned().unwrap_or(Value::Null),
                    &Value::String("~".into()),
                ),
            ),
            &super::runtime::strict_equal(
                &args.first().cloned().unwrap_or(Value::Null),
                &Value::String(".".into()),
            ),
        ),
        &super::runtime::strict_equal(
            &args.first().cloned().unwrap_or(Value::Null),
            &Value::String("..".into()),
        ),
    )
}
