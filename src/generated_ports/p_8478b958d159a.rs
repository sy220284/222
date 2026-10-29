//! Rust exports derived from original: src/utils/streamlinedTransform.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::{Value,json};

pub fn shouldIncludeInStreamlined(args:&[Value])->Value {super::runtime::or(&super::runtime::strict_equal(&super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null),"type"),&Value::String("assistant".into())),&super::runtime::strict_equal(&super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null),"type"),&Value::String("result".into())))}
