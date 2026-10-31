//! Rust exports derived from original: src/hooks/useFileHistorySnapshotInit.ts
//!
//! One-shot restore contract: when file history is enabled, the first run
//! restores state from the initial snapshot log and never re-runs. The
//! fileHistory accessors port with the file-history chain; the guard
//! semantics below are the hook's full decision logic.

/// The one-shot initialization decision (enabled gate + initialized latch).
pub fn should_initialize(file_history_enabled: bool, already_initialized: bool) -> bool {
    file_history_enabled && !already_initialized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard_matches_hook() {
        assert!(should_initialize(true, false));
        assert!(!should_initialize(true, true));
        assert!(!should_initialize(false, false));
    }
}
