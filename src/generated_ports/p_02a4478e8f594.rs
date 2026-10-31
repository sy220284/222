//! Rust exports derived from original: src/ink/hooks/use-animation-frame.ts
//!
//! Shared-clock animation contract: every instance subscribes to one clock
//! so animations stay in sync; the clock runs only while at least one
//! keepAlive subscriber exists; passing null pauses (time freezes at the
//! last value and resumes from current clock time). The ClockContext
//! subscription and DOMElement ref live in the ink runtime host.

/// Frame progression between clock ticks: time jumps straight to the
/// current clock value when paused and resumed (no drift accumulation).
pub fn resumed_time(last_frozen_ms: Option<u64>, now_ms: u64) -> u64 {
    match last_frozen_ms {
        Some(frozen) if frozen > now_ms => frozen,
        _ => now_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resume_semantics() {
        assert_eq!(resumed_time(None, 100), 100);
        assert_eq!(resumed_time(Some(80), 100), 100);
        assert_eq!(resumed_time(Some(120), 100), 120); // clock went backwards: keep frozen
    }
}
