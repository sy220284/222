//! Rust exports derived from original: src/hooks/useMinDisplayTime.ts
//!
//! The throttle decision is fully equivalent: each distinct value stays
//! visible for at least `minMs` (neither debounce nor throttle). The delayed
//! swap timer stays with the terminal runtime (hook host pending).

/// Decision for a value swap observed `elapsed_ms` after the last shown swap:
/// show immediately, or wait the remainder. Mirrors the effect body.
pub fn min_display_decision(elapsed_ms: u64, min_ms: u64) -> Result<(), u64> {
    if elapsed_ms >= min_ms {
        Ok(())
    } else {
        Err(min_ms - elapsed_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_or_remainder() {
        assert_eq!(min_display_decision(500, 300), Ok(()));
        assert_eq!(min_display_decision(100, 300), Err(200));
    }
}
