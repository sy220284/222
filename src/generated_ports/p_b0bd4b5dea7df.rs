//! Rust exports derived from original: src/services/policyLimits/types.ts
//!
//! Policy limits API response schema and fetch result. The zod object maps
//! to serde types; the lazy schema wrapper becomes plain structural
//! validation (only blocked policies are included — an absent key allows).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// `PolicyLimitsResponse` — restrictions keyed by policy name.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct PolicyLimitsResponse {
    #[serde(default)]
    pub restrictions: HashMap<String, Restriction>,
}

/// `{ allowed: boolean }` per policy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Restriction {
    pub allowed: bool,
}

/// `PolicyLimitsResponse['restrictions']` alias parity.
pub type PolicyRestrictions = HashMap<String, Restriction>;

/// `PolicyLimitsFetchResult`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PolicyLimitsFetchResult {
    pub success: bool,
    /// null means 304 Not Modified (cache is valid).
    pub restrictions: Option<Option<PolicyRestrictions>>,
    pub etag: Option<String>,
    pub error: Option<String>,
    /// If true, don't retry on failure (e.g., auth errors).
    pub skip_retry: bool,
}

/// Structural validation equivalent of the zod record-of-objects schema:
/// every restriction value must carry a boolean `allowed` field.
pub fn validate_policy_limits(value: &serde_json::Value) -> Result<(), String> {
    let obj = value
        .as_object()
        .ok_or_else(|| "expected object".to_string())?;
    let restrictions = obj
        .get("restrictions")
        .ok_or_else(|| "missing restrictions".to_string())?;
    let map = restrictions
        .as_object()
        .ok_or_else(|| "restrictions must be an object".to_string())?;
    for (key, entry) in map {
        let allowed = entry
            .get("allowed")
            .ok_or_else(|| format!("policy {key}: missing allowed"))?;
        if !allowed.is_boolean() {
            return Err(format!("policy {key}: allowed must be a boolean"));
        }
    }
    Ok(())
}
