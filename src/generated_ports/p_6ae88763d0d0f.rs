//! Rust exports derived from original: src/utils/aws.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::{Value,json};

pub fn isAwsCredentialsProviderError(args:&[Value])->Value {super::runtime::strict_equal(&super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null),"name"),&Value::String("CredentialsProviderError".into()))}
