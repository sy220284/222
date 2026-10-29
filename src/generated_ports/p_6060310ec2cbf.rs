//! Rust exports derived from original: src/utils/plugins/pluginStartupCheck.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::{Value,json};

pub fn isPersistableScope(args:&[Value])->Value {super::runtime::strict_unequal(&args.get(0).cloned().unwrap_or(Value::Null),&Value::String("flag".into()))}
