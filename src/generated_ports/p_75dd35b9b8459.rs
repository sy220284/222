//! Rust exports derived from original: src/components/tasks/taskStatusUtils.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::{Value,json};

pub fn isTerminalStatus(args:&[Value])->Value {super::runtime::or(&super::runtime::or(&super::runtime::strict_equal(&args.get(0).cloned().unwrap_or(Value::Null),&Value::String("completed".into())),&super::runtime::strict_equal(&args.get(0).cloned().unwrap_or(Value::Null),&Value::String("failed".into()))),&super::runtime::strict_equal(&args.get(0).cloned().unwrap_or(Value::Null),&Value::String("killed".into())))}
