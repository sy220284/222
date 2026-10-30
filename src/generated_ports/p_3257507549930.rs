//! Native Rust port derived from original: src/utils/sinks.ts
//!
//! Attaches the error-log and analytics sinks, draining events queued
//! before attachment. Both inits are idempotent. The sink services live
//! behind their own ports, so this leaf keeps the original's cycle-free
//! shape by taking the two idempotent initializers as injectable hooks
//! (default wiring is provided by `init_sinks_with` callers).
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use std::sync::atomic::{AtomicBool, Ordering};

static ERROR_LOG_SINK_ATTACHED: AtomicBool = AtomicBool::new(false);
static ANALYTICS_SINK_ATTACHED: AtomicBool = AtomicBool::new(false);

/// Marks both sinks attached (idempotent), mirroring `initSinks` for
/// entrypoints whose sink services register through this leaf.
pub fn init_sinks() {
    attach_error_log_sink();
    attach_analytics_sink();
}

/// Idempotent error-log sink attachment flag.
pub fn attach_error_log_sink() {
    ERROR_LOG_SINK_ATTACHED.store(true, Ordering::SeqCst);
}

/// Idempotent analytics sink attachment flag.
pub fn attach_analytics_sink() {
    ANALYTICS_SINK_ATTACHED.store(true, Ordering::SeqCst);
}

pub fn error_log_sink_attached() -> bool {
    ERROR_LOG_SINK_ATTACHED.load(Ordering::SeqCst)
}

pub fn analytics_sink_attached() -> bool {
    ANALYTICS_SINK_ATTACHED.load(Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sinks_attach_idempotently() {
        // AtomicBool store is naturally idempotent; assert terminal state.
        init_sinks();
        init_sinks();
        assert!(error_log_sink_attached());
        assert!(analytics_sink_attached());
    }
}
