//! Rust port of `src/query/config.ts` (2.1.88 original implementation).
//!
//! A query turn snapshots its environment-derived configuration exactly once
//! at entry so every loop iteration observes consistent values. The original
//! notes that server-side rollout `feature()` gates are intentionally excluded from the
//! snapshot (their contract admits staleness) — the streaming-tool-execution
//! gate is likewise resolved where the streaming path runs, so only the
//! environment gates are snapshotted here, mirroring `buildQueryConfig()`
//! field-for-field. Per the established migration convention, the original
//! compile-time feature gates become process-level switches.

use crate::env_runtime::is_env_truthy;
use uuid::Uuid;

/// `QueryGates` — feature gates resolved once per `query()` entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueryGates {
    /// streaming tool execution (server rollout gate) — the streaming tool execution path
    /// (`src/services/tools/StreamingToolExecutor.ts`) is not yet ported, so
    /// the loop always resolves this gate to `false` today, exactly like a
    /// rolled-off vendor rollout service gate would.
    pub streaming_tool_execution: bool,
    /// `ATLAS_EMIT_TOOL_USE_SUMMARIES` (env truthy).
    pub emit_tool_use_summaries: bool,
    /// `process.env.USER_TYPE === 'ant'` — vendor-internal cohort signal kept
    /// for behavioral parity; never grants access by itself.
    pub is_ant: bool,
    /// `!isEnvTruthy(process.env.ATLAS_DISABLE_FAST_MODE)` — inlined
    /// from `fastMode.ts` in the original to avoid its heavy module graph.
    pub fast_mode_enabled: bool,
}

/// `QueryConfig` — immutable snapshot taken once at `query()` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryConfig {
    pub session_id: Uuid,
    pub gates: QueryGates,
}

/// `buildQueryConfig()`.
pub fn build_query_config(session_id: Uuid) -> QueryConfig {
    QueryConfig {
        session_id,
        gates: QueryGates {
            streaming_tool_execution: false,
            emit_tool_use_summaries: is_env_truthy(
                std::env::var("ATLAS_EMIT_TOOL_USE_SUMMARIES")
                    .ok()
                    .as_deref(),
            ),
            is_ant: std::env::var("USER_TYPE").ok().as_deref() == Some("ant"),
            fast_mode_enabled: !is_env_truthy(
                std::env::var("ATLAS_DISABLE_FAST_MODE").ok().as_deref(),
            ),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_gates_follow_isenvtruthy_semantics() {
        // fast mode is on unless explicitly disabled
        std::env::remove_var("ATLAS_DISABLE_FAST_MODE");
        assert!(build_query_config(Uuid::nil()).gates.fast_mode_enabled);
        for truthy in ["1", "true", "TRUE", " yes ", "on"] {
            std::env::set_var("ATLAS_DISABLE_FAST_MODE", truthy);
            assert!(!build_query_config(Uuid::nil()).gates.fast_mode_enabled);
        }
        for falsy in ["0", "false", "no", "off", ""] {
            std::env::set_var("ATLAS_DISABLE_FAST_MODE", falsy);
            assert!(build_query_config(Uuid::nil()).gates.fast_mode_enabled);
        }
        std::env::remove_var("ATLAS_DISABLE_FAST_MODE");

        // summary emission is opt-in truthy
        std::env::remove_var("ATLAS_EMIT_TOOL_USE_SUMMARIES");
        assert!(
            !build_query_config(Uuid::nil())
                .gates
                .emit_tool_use_summaries
        );
        std::env::set_var("ATLAS_EMIT_TOOL_USE_SUMMARIES", "1");
        assert!(
            build_query_config(Uuid::nil())
                .gates
                .emit_tool_use_summaries
        );
        std::env::remove_var("ATLAS_EMIT_TOOL_USE_SUMMARIES");
        assert!(
            !build_query_config(Uuid::nil())
                .gates
                .emit_tool_use_summaries
        );
    }

    #[test]
    fn is_ant_requires_exact_user_type() {
        std::env::remove_var("USER_TYPE");
        assert!(!build_query_config(Uuid::nil()).gates.is_ant);
        std::env::set_var("USER_TYPE", "external");
        assert!(!build_query_config(Uuid::nil()).gates.is_ant);
        std::env::set_var("USER_TYPE", "ant");
        assert!(build_query_config(Uuid::nil()).gates.is_ant);
        std::env::remove_var("USER_TYPE");
    }

    #[test]
    fn streaming_tool_execution_resolves_off_until_ported() {
        std::env::remove_var("ATLAS_EMIT_TOOL_USE_SUMMARIES");
        std::env::remove_var("ATLAS_DISABLE_FAST_MODE");
        std::env::remove_var("USER_TYPE");
        assert!(
            !build_query_config(Uuid::nil())
                .gates
                .streaming_tool_execution
        );
    }

    #[test]
    fn session_id_snapshot_is_carried() {
        let id = Uuid::new_v4();
        assert_eq!(build_query_config(id).session_id, id);
    }
}
