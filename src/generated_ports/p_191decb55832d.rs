//! Rust exports derived from original: src/utils/plugins/marketplaceHelpers.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EmptyMarketplaceReason {
    #[serde(rename = "git-not-installed")]
    git_not_installed,
    #[serde(rename = "all-blocked-by-policy")]
    all_blocked_by_policy,
    #[serde(rename = "policy-restricts-sources")]
    policy_restricts_sources,
    #[serde(rename = "all-marketplaces-failed")]
    all_marketplaces_failed,
    #[serde(rename = "no-marketplaces-configured")]
    no_marketplaces_configured,
    #[serde(rename = "all-plugins-installed")]
    all_plugins_installed,
}

use serde_json::json;

pub fn createPluginId(args: &[Value]) -> Value {
    super::runtime::concat(&[
        Value::String("".into()),
        args.get(0).cloned().unwrap_or(Value::Null),
        Value::String("@".into()),
        args.get(1).cloned().unwrap_or(Value::Null),
        Value::String("".into()),
    ])
}
