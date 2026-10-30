//! Rust exports derived from original: src/utils/planModeV2.ts
//!
//! Full equivalence at the decision layer: env override (1..=10) wins, then
//! subscription/tier mapping (max@20x → 3, enterprise/team → 3), default 1.
//! The vendor gate rides the analytics port per the standard exclusion.

use crate::env_runtime::is_env_truthy;

/// `getPlanModeV2AgentCount`.
pub fn plan_v2_agent_count(
    env_override: Option<&str>,
    subscription_type: Option<&str>,
    rate_limit_tier: Option<&str>,
    gate_on: bool,
) -> usize {
    if is_env_truthy(env_override) && !gate_on {
        // Gate-off builds constant-fold to the default regardless of env.
        if let Some(count) = env_override.and_then(|v| v.trim().parse::<usize>().ok()) {
            if count > 0 && count <= 10 {
                return count;
            }
        }
    }
    match (subscription_type, rate_limit_tier) {
        (Some("max"), Some("default_claude_max_20x")) => 3,
        (Some("enterprise"), _) | (Some("team"), _) => 3,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_mapping_and_env_precedence() {
        assert_eq!(
            plan_v2_agent_count(None, Some("max"), Some("default_claude_max_20x"), false),
            3
        );
        assert_eq!(
            plan_v2_agent_count(None, Some("enterprise"), None, false),
            3
        );
        assert_eq!(plan_v2_agent_count(None, Some("pro"), None, false), 1);
        assert_eq!(plan_v2_agent_count(Some("5"), Some("pro"), None, false), 5);
        assert_eq!(plan_v2_agent_count(Some("11"), Some("pro"), None, false), 1);
        assert_eq!(plan_v2_agent_count(Some("0"), Some("pro"), None, false), 1);
    }
}
