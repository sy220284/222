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
    build_query_config_with_env(session_id, |name| std::env::var(name).ok())
}

/// The original query() takes a snapshot once. Passing the environment
/// lookup in makes that behavior deterministic under concurrently running
/// CLI/SDK tasks, and lets tests exercise it without changing process globals.
fn build_query_config_with_env(
    session_id: Uuid,
    mut get: impl FnMut(&str) -> Option<String>,
) -> QueryConfig {
    let streaming = get("ATLAS_STREAMING_TOOL_EXECUTION");
    let summaries = get("ATLAS_EMIT_TOOL_USE_SUMMARIES");
    let user_type = get("USER_TYPE");
    let fast_disabled = get("ATLAS_DISABLE_FAST_MODE");
    QueryConfig {
        session_id,
        gates: QueryGates {
            streaming_tool_execution: is_env_truthy(streaming.as_deref()),
            emit_tool_use_summaries: is_env_truthy(summaries.as_deref()),
            is_ant: user_type.as_deref() == Some("ant"),
            fast_mode_enabled: !is_env_truthy(fast_disabled.as_deref()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_value(key: &str, value: &str) -> QueryConfig {
        build_query_config_with_env(Uuid::nil(), |name| {
            (name == key).then(|| value.to_owned())
        })
    }

    #[test]
    fn env_gates_follow_isenvtruthy_semantics() {
        assert!(build_query_config_with_env(Uuid::nil(), |_| None)
            .gates
            .fast_mode_enabled);
        for truthy in ["1", "true", "TRUE", " yes ", "on"] {
            assert!(!with_value("ATLAS_DISABLE_FAST_MODE", truthy)
                .gates
                .fast_mode_enabled);
            assert!(with_value("ATLAS_EMIT_TOOL_USE_SUMMARIES", truthy)
                .gates
                .emit_tool_use_summaries);
        }
        for falsy in ["0", "false", "no", "off", ""] {
            assert!(with_value("ATLAS_DISABLE_FAST_MODE", falsy)
                .gates
                .fast_mode_enabled);
            assert!(!with_value("ATLAS_EMIT_TOOL_USE_SUMMARIES", falsy)
                .gates
                .emit_tool_use_summaries);
        }
        assert!(!build_query_config_with_env(Uuid::nil(), |_| None)
            .gates
            .emit_tool_use_summaries);
    }

    #[test]
    fn is_ant_requires_exact_user_type() {
        assert!(!build_query_config_with_env(Uuid::nil(), |_| None)
            .gates
            .is_ant);
        assert!(!with_value("USER_TYPE", "external").gates.is_ant);
        assert!(with_value("USER_TYPE", "ant").gates.is_ant);
    }

    #[test]
    fn streaming_tool_execution_follows_process_switch() {
        assert!(!build_query_config_with_env(Uuid::nil(), |_| None)
            .gates
            .streaming_tool_execution);
        assert!(with_value("ATLAS_STREAMING_TOOL_EXECUTION", "1")
            .gates
            .streaming_tool_execution);
        assert!(!with_value("ATLAS_STREAMING_TOOL_EXECUTION", "off")
            .gates
            .streaming_tool_execution);
    }

    #[test]
    fn session_id_snapshot_is_carried() {
        let id = Uuid::new_v4();
        assert_eq!(build_query_config(id).session_id, id);
    }
}
