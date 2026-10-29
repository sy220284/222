//! Rust exports derived from original: src/utils/proxy.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::{Value,json};

pub fn getProxyUrl(args:&[Value])->Value {super::runtime::or(&super::runtime::or(&super::runtime::or(&super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null),"https_proxy"),&super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null),"HTTPS_PROXY")),&super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null),"http_proxy")),&super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null),"HTTP_PROXY"))}
pub fn getNoProxy(args:&[Value])->Value {super::runtime::or(&super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null),"no_proxy"),&super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null),"NO_PROXY"))}
