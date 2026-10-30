//! Rust exports derived from original: src/utils/frontmatterParser.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FrontmatterShell {
    #[serde(rename = "bash")]
    bash,
    #[serde(rename = "powershell")]
    powershell,
}

pub fn parseBooleanFrontmatter(args: &[Value]) -> Value {
    super::runtime::or(
        &super::runtime::strict_equal(
            &args.first().cloned().unwrap_or(Value::Null),
            &Value::Bool(true),
        ),
        &super::runtime::strict_equal(
            &args.first().cloned().unwrap_or(Value::Null),
            &Value::String("true".into()),
        ),
    )
}
