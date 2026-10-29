//! Rust exports derived from original: src/utils/swarm/backends/detection.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub fn IT2_COMMAND() -> Value {
    serde_json::from_str(r#""it2""#).expect("Valid source constant")
}
