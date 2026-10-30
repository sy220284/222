//! Rust exports derived from original: src/hooks/useBlink.ts
//!
//! Pure blink-state derivation is fully equivalent; the animation-frame
//! subscription stays with the terminal runtime (hook host pending).

/// `BLINK_INTERVAL_MS`
pub const BLINK_INTERVAL_MS: u64 = 600;

/// Original derivation: all instances blink together because they derive
/// state from the same animation clock — visible on even half-cycles.
pub fn blink_is_visible(time_ms: u64, interval_ms: u64) -> bool {
    (time_ms / interval_ms) % 2 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn even_half_cycles_are_visible() {
        assert!(blink_is_visible(0, 600));
        assert!(!blink_is_visible(600, 600));
        assert!(blink_is_visible(1200, 600));
    }
}
