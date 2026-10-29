//! Rust exports derived from original: src/utils/cleanup.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::{Value,json};

pub fn addCleanupResults(args:&[Value])->Value {{let mut map=serde_json::Map::new();map.insert("messages".into(),super::runtime::add(&super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null),"messages"),&super::runtime::property(&args.get(1).cloned().unwrap_or(Value::Null),"messages")));map.insert("errors".into(),super::runtime::add(&super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null),"errors"),&super::runtime::property(&args.get(1).cloned().unwrap_or(Value::Null),"errors"))); Value::Object(map)}}
