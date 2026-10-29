//! Rust exports derived from original: src/upstreamproxy/upstreamproxy.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub fn SESSION_TOKEN_PATH() -> Value {
    serde_json::from_str(r#""/run/ccr/session_token""#).expect("Valid source constant")
}
