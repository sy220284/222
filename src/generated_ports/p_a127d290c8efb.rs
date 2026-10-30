//! Rust exports derived from original: src/utils/analyzeContext.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub fn TOOL_TOKEN_COUNT_OVERHEAD() -> Value {
    serde_json::from_str(r#"500"#).expect("Valid source constant")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeferredBuiltinTool {
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "tokens")]
    pub tokens: f64,
    #[serde(rename = "isLoaded")]
    pub isLoaded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemToolDetail {
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "tokens")]
    pub tokens: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemPromptSectionDetail {
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "tokens")]
    pub tokens: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextData {
    #[serde(rename = "categories")]
    pub categories: Vec<Value>,
    #[serde(rename = "totalTokens")]
    pub totalTokens: f64,
    #[serde(rename = "maxTokens")]
    pub maxTokens: f64,
    #[serde(rename = "rawMaxTokens")]
    pub rawMaxTokens: f64,
    #[serde(rename = "percentage")]
    pub percentage: f64,
    #[serde(rename = "gridRows")]
    pub gridRows: Vec<Vec<Value>>,
    #[serde(rename = "model")]
    pub model: String,
    #[serde(rename = "memoryFiles")]
    pub memoryFiles: Vec<Value>,
    #[serde(rename = "mcpTools")]
    pub mcpTools: Vec<Value>,
    #[serde(rename = "deferredBuiltinTools", default)]
    pub deferredBuiltinTools: Option<Vec<Value>>,
    #[serde(rename = "systemTools", default)]
    pub systemTools: Option<Vec<Value>>,
    #[serde(rename = "systemPromptSections", default)]
    pub systemPromptSections: Option<Vec<Value>>,
    #[serde(rename = "agents")]
    pub agents: Vec<Value>,
    #[serde(rename = "slashCommands", default)]
    pub slashCommands: Option<Value>,
    #[serde(rename = "skills", default)]
    pub skills: Option<Value>,
    #[serde(rename = "autoCompactThreshold", default)]
    pub autoCompactThreshold: Option<f64>,
    #[serde(rename = "isAutoCompactEnabled")]
    pub isAutoCompactEnabled: bool,
    #[serde(rename = "messageBreakdown", default)]
    pub messageBreakdown: Option<Value>,
    #[serde(rename = "apiUsage")]
    pub apiUsage: Value,
}
