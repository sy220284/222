//! Rust exports derived from original: src/hooks/useSwarmInitialization.ts
//!
//! Swarm init contract (conditionally loaded for dead-code elimination):
//! resumed teammate sessions rehydrate team context + hooks from session
//! state; fresh spawns initialize hooks only. Both writes go through
//! setAppState once. Gate = ENABLE_AGENT_SWARMS.

/// Initialization plan for a teammate session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwarmInit {
    Skip,
    /// Resumed: rehydrate team context from session files.
    Resume,
    /// Fresh spawn: hooks only.
    Fresh,
}

/// Which initialization path applies.
pub fn init_plan(gate_enabled: bool, is_resumed: bool) -> SwarmInit {
    if !gate_enabled {
        SwarmInit::Skip
    } else if is_resumed {
        SwarmInit::Resume
    } else {
        SwarmInit::Fresh
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_matrix() {
        assert_eq!(init_plan(false, true), SwarmInit::Skip);
        assert_eq!(init_plan(true, true), SwarmInit::Resume);
        assert_eq!(init_plan(true, false), SwarmInit::Fresh);
    }
}
