//! Rust exports derived from original: src/utils/model/antModels.ts
//!
//! AntModel shape with serde parity (alias/model/label plus optional effort,
//! context window and thinking overrides). The rollout-config plumbing is a
//! vendor surface and stays out; the data shape is fully equivalent.

use serde::{Deserialize, Serialize};

/// `AntModel`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AntModel {
    pub alias: String,
    pub model: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_effort_value: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_effort_level: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_max_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upper_max_tokens_limit: Option<u32>,
    /// Model defaults to adaptive thinking and rejects thinking: disabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub always_on_thinking: Option<bool>,
}
