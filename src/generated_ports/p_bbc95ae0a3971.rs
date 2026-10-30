//! Rust exports derived from original: src/utils/plugins/officialMarketplaceStartupCheck.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum OfficialMarketplaceSkipReason {
    #[serde(rename = "already_attempted")]
    already_attempted,
    #[serde(rename = "already_installed")]
    already_installed,
    #[serde(rename = "policy_blocked")]
    policy_blocked,
    #[serde(rename = "git_unavailable")]
    git_unavailable,
    #[serde(rename = "gcs_unavailable")]
    gcs_unavailable,
    #[serde(rename = "unknown")]
    unknown,
}
