//! Rust exports derived from original: src/ink/hooks/use-interval.ts
//!
//! Shared-clock subscription semantics: returns the clock time updating at
//! `intervalMs`, subscribing as non-keepAlive (never keeps the clock alive
//! on its own). Pure gating math here; the clock itself lives in the ink
//! runtime.

/// Gate for one clock tick: does the elapsed delta reach the interval?
/// Mirrors `now - lastUpdate >= intervalMs` with the lastUpdate advance.
pub fn tick_reaches_interval(last_update_ms: u64, now_ms: u64, interval_ms: u64) -> bool {
    now_ms.saturating_sub(last_update_ms) >= interval_ms
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_gate() {
        assert!(!tick_reaches_interval(100, 149, 50));
        assert!(tick_reaches_interval(100, 150, 50));
        assert!(tick_reaches_interval(100, 90, 50)); // saturating: 0
    }
}
