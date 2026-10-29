//! Rust exports derived from original: src/utils/agentContext.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::{json, Value};

pub fn isSubagentContext(args: &[Value]) -> Value {
    super::runtime::strict_equal(
        &super::runtime::property(&args.first().cloned().unwrap_or(Value::Null), "agentType"),
        &Value::String("subagent".into()),
    )
}
