//! Rust exports derived from original: src/utils/stringUtils.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::{Value,json};

pub fn capitalize(args:&[Value])->Value {super::runtime::add(&super::runtime::call_method(&super::runtime::call_method(&args.get(0).cloned().unwrap_or(Value::Null),"charAt",&[json!(0)]),"toUpperCase",&[]),&super::runtime::call_method(&args.get(0).cloned().unwrap_or(Value::Null),"slice",&[json!(1)]))}
pub fn plural(args:&[Value])->Value {if super::runtime::truthy(&super::runtime::strict_equal(&args.get(0).cloned().unwrap_or(Value::Null),&json!(1))){args.get(1).cloned().unwrap_or(Value::Null)}else{args.get(2).cloned().unwrap_or(Value::Null)}}
