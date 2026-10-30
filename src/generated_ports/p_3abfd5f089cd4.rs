//! Rust exports derived from original: src/utils/planModeV2.ts
//!
//! plan v2 is not part of this build's active feature set: the vendor
//! rollout config reads closed, so `getPlanModeV2AgentCount` reduces to the
//! documented default of 1 (the same value the original yields when no
//! override/subscription tier applies). Env override and tier helpers port
//! with their chains.

/// `getPlanModeV2AgentCount` — external build default path.
pub fn get_plan_mode_v2_agent_count() -> u32 {
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_path() {
        assert_eq!(get_plan_mode_v2_agent_count(), 1);
    }
}
