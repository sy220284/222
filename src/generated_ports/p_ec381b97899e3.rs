//! Rust exports derived from original: src/components/permissions/PowerShellPermissionRequest/powershellToolUseOptions.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PowerShellToolUseOption {
    #[serde(rename = "yes")]
    yes,
    #[serde(rename = "yes-apply-suggestions")]
    yes_apply_suggestions,
    #[serde(rename = "yes-prefix-edited")]
    yes_prefix_edited,
    #[serde(rename = "no")]
    no,
}
