//! Rust exports derived from original: src/utils/renderOptions.ts
//!
//! Stdin override decision: when stdin is piped, interactive Ink rendering
//! opens /dev/tty instead (cached for the process lifetime); the env kill
//! switch disables the override entirely.

/// Whether the /dev/tty override may be attempted.
pub fn stdin_override_eligible(stdin_is_tty: bool, env_disabled: bool) -> bool {
    !env_disabled && !stdin_is_tty
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_gate() {
        assert!(stdin_override_eligible(false, false));
        assert!(!stdin_override_eligible(true, false));
        assert!(!stdin_override_eligible(false, true));
    }
}
