//! Rust exports derived from original: src/components/permissions/BashPermissionRequest/bashToolUseOptions.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BashToolUseOption {
    #[serde(rename = "yes")]
    yes,
    #[serde(rename = "yes-apply-suggestions")]
    yes_apply_suggestions,
    #[serde(rename = "yes-prefix-edited")]
    yes_prefix_edited,
    #[serde(rename = "yes-classifier-reviewed")]
    yes_classifier_reviewed,
    #[serde(rename = "no")]
    no,
}
