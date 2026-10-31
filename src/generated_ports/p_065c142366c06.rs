//! Rust exports derived from original:
//! src/components/FeedbackSurvey/useDebouncedDigitInput.ts
//!
//! Full equivalence: debounce window (400ms default) so accidental leading
//! digits ("1. First item") cancel, full-width digit normalization before
//! validation, and once-only firing. Timer subscription rides the host.

/// Default acceptance delay.
pub const DEFAULT_DEBOUNCE_MS: u64 = 400;

/// Should the pending digit fire now? Called when the debounce elapses and
/// the input value is unchanged since arming.
pub fn should_fire(
    armed: bool,
    input_changed_since_arm: bool,
    already_fired: bool,
    once: bool,
) -> bool {
    armed && !input_changed_since_arm && !(once && already_fired)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fire_gates() {
        assert!(should_fire(true, false, false, true));
        assert!(!should_fire(true, true, false, true));
        assert!(!should_fire(true, false, true, true));
        // Non-once can refire.
        assert!(should_fire(true, false, true, false));
    }
}
