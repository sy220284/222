//! Rust exports derived from original: src/utils/swarm/backends/it2Setup.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PythonPackageManager {
    #[serde(rename = "uvx")]
    uvx,
    #[serde(rename = "pipx")]
    pipx,
    #[serde(rename = "pip")]
    pip,
}

pub fn getPythonApiInstructions(args: &[Value]) -> Value {
    Value::Array(vec![
        Value::String("Almost done! Enable the Python API in iTerm2:".into()),
        Value::String("".into()),
        Value::String("  iTerm2 → Settings → General → Magic → Enable Python API".into()),
        Value::String("".into()),
        Value::String("After enabling, you may need to restart iTerm2.".into()),
    ])
}
