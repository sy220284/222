//! Rust exports derived from original: src/utils/plugins/mcpbHandler.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::{Value,json};

pub fn isMcpbSource(args:&[Value])->Value {super::runtime::or(&super::runtime::call_method(&args.get(0).cloned().unwrap_or(Value::Null),"endsWith",&[Value::String(".mcpb".into())]),&super::runtime::call_method(&args.get(0).cloned().unwrap_or(Value::Null),"endsWith",&[Value::String(".dxt".into())]))}
