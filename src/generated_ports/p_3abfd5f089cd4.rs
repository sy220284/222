//! Rust exports derived from original: src/utils/planModeV2.ts
//!
//! Full equivalence at the decision layer: env override (1..=10) wins, then
//! subscription/tier mapping (max@20x → 3, enterprise/team → 3), default 1.
//! The vendor gate rides the analytics port per the standard exclusion.

/// `getPlanModeV2AgentCount`.
pub fn plan_v2_agent_count(
    env_override: Option<&str>,
    subscription_type: Option<&str>,
    rate_limit_tier: Option<&str>,
) -> usize {
    // Upstream: if the env variable is set, parseInt(value, 10) accepts
    // a signed decimal prefix. A valid positive result takes precedence.
    if let Some(raw) = env_override {
        let prefix = raw.trim_start();
        let prefix = prefix.strip_prefix('+').unwrap_or(prefix);
        if !prefix.starts_with('-') {
            let digits = prefix
                .chars()
                .take_while(|ch| ch.is_ascii_digit())
                .collect::<String>();
            if let Ok(count) = digits.parse::<usize>() {
                if (1..=10).contains(&count) {
                    return count;
                }
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
            plan_v2_agent_count(None, Some("max"), Some("default_claude_max_20x")),
            3
        );
        assert_eq!(
            plan_v2_agent_count(None, Some("enterprise"), None),
            3
        );
        assert_eq!(plan_v2_agent_count(None, Some("pro"), None), 1);
        assert_eq!(plan_v2_agent_count(Some("5"), Some("pro"), None), 5);
        assert_eq!(plan_v2_agent_count(Some("11"), Some("pro"), None), 1);
        assert_eq!(plan_v2_agent_count(Some("0"), Some("pro"), None), 1);
        assert_eq!(plan_v2_agent_count(Some("  +5agents"), Some("pro"), None), 5);
        assert_eq!(plan_v2_agent_count(Some("10.5"), Some("pro"), None), 10);
        assert_eq!(plan_v2_agent_count(Some("0x10"), Some("pro"), None), 1);
        assert_eq!(plan_v2_agent_count(Some("-5"), Some("pro"), None), 1);
        assert_eq!(plan_v2_agent_count(Some("not-a-number"), Some("team"), None), 3);
    }
}
