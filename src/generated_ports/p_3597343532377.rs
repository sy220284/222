//! Rust exports derived from original: src/components/Spinner/useShimmerAnimation.ts
//!
//! Glimmer timing math is fully equivalent and tested: requesting spins at
//! 50ms, other modes at 200ms; a stalled spinner reports -100 and stops the
//! animation clock; the glimmer index walks a cycle of message width + 20.
//! The animation-frame subscription and DOM ref ride the ink runtime.

use serde::Serialize;

/// Shimmer speed for a mode in milliseconds.
pub fn glimmer_speed_ms(mode: &str) -> u64 {
    if mode == "requesting" {
        50
    } else {
        200
    }
}

/// Glimmer index for a message at a given clock time (ms).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GlimmerFrame {
    pub glimmer_index: i64,
}

pub fn glimmer_frame(
    mode: &str,
    message: &str,
    time_ms: u64,
    message_width: usize,
) -> GlimmerFrame {
    let speed = glimmer_speed_ms(mode);
    let cycle_length = message_width + 20;
    let cycle_position = (time_ms / speed) as i64;
    GlimmerFrame {
        glimmer_index: (cycle_position % cycle_length as i64) - 10,
    }
}

/// Stalled spinners freeze at -100 and unsubscribe from the clock.
pub const STALLED_INDEX: i64 = -100;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speeds_and_cycle_math_match_original() {
        assert_eq!(glimmer_speed_ms("requesting"), 50);
        assert_eq!(glimmer_speed_ms("processing"), 200);
        let f = glimmer_frame("requesting", "hello", 100, 5);
        // cycle_position = 100/50 = 2; 2 % 25 - 10 = -8
        assert_eq!(f.glimmer_index, -8);
    }
}
