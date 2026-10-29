//! Rust exports derived from original: src/services/mockRateLimits.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MockHeaderKey {
    #[serde(rename = "status")]
    status,
    #[serde(rename = "reset")]
    reset,
    #[serde(rename = "claim")]
    claim,
    #[serde(rename = "overage-status")]
    overage_status,
    #[serde(rename = "overage-reset")]
    overage_reset,
    #[serde(rename = "overage-disabled-reason")]
    overage_disabled_reason,
    #[serde(rename = "fallback")]
    fallback,
    #[serde(rename = "fallback-percentage")]
    fallback_percentage,
    #[serde(rename = "retry-after")]
    retry_after,
    #[serde(rename = "5h-utilization")]
    n_5h_utilization,
    #[serde(rename = "5h-reset")]
    n_5h_reset,
    #[serde(rename = "5h-surpassed-threshold")]
    n_5h_surpassed_threshold,
    #[serde(rename = "7d-utilization")]
    n_7d_utilization,
    #[serde(rename = "7d-reset")]
    n_7d_reset,
    #[serde(rename = "7d-surpassed-threshold")]
    n_7d_surpassed_threshold,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MockScenario {
    #[serde(rename = "normal")]
    normal,
    #[serde(rename = "session-limit-reached")]
    session_limit_reached,
    #[serde(rename = "approaching-weekly-limit")]
    approaching_weekly_limit,
    #[serde(rename = "weekly-limit-reached")]
    weekly_limit_reached,
    #[serde(rename = "overage-active")]
    overage_active,
    #[serde(rename = "overage-warning")]
    overage_warning,
    #[serde(rename = "overage-exhausted")]
    overage_exhausted,
    #[serde(rename = "out-of-credits")]
    out_of_credits,
    #[serde(rename = "org-zero-credit-limit")]
    org_zero_credit_limit,
    #[serde(rename = "org-spend-cap-hit")]
    org_spend_cap_hit,
    #[serde(rename = "member-zero-credit-limit")]
    member_zero_credit_limit,
    #[serde(rename = "seat-tier-zero-credit-limit")]
    seat_tier_zero_credit_limit,
    #[serde(rename = "opus-limit")]
    opus_limit,
    #[serde(rename = "opus-warning")]
    opus_warning,
    #[serde(rename = "sonnet-limit")]
    sonnet_limit,
    #[serde(rename = "sonnet-warning")]
    sonnet_warning,
    #[serde(rename = "fast-mode-limit")]
    fast_mode_limit,
    #[serde(rename = "fast-mode-short-limit")]
    fast_mode_short_limit,
    #[serde(rename = "extra-usage-required")]
    extra_usage_required,
    #[serde(rename = "clear")]
    clear,
}
