//! Rust exports derived from original: src/utils/computerUse/common.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
pub fn COMPUTER_USE_MCP_SERVER_NAME()->Value{serde_json::from_str(r#""computer-use""#).expect("Valid source constant")}

pub fn CLI_CU_CAPABILITIES()->Value{serde_json::from_str(r#"{"screenshotFiltering":"native","platform":"darwin"}"#).expect("Valid source constant")}
