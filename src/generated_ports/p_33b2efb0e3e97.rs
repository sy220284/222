//! Rust exports derived from original: src/hooks/useExitOnCtrlCD.ts
//!
//! Time-based double-press exit state machine — fully portable. First press
//! arms with the key name shown; second press within the window exits; the
//! other key resets the armed state.

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExitState {
    pub pending: bool,
    pub key_name: Option<&'static str>,
}

/// `handleKeyPress` — `armed_key` is the currently armed key (if any).
pub fn handle_key_press(armed_key: Option<&'static str>, key: &'static str) -> (ExitState, bool) {
    match armed_key {
        Some(armed) if armed == key => {
            // Second press within the window: exit.
            (
                ExitState {
                    pending: false,
                    key_name: None,
                },
                true,
            )
        }
        _ => (
            ExitState {
                pending: true,
                key_name: Some(key),
            },
            false,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn double_press_and_reset() {
        let (armed, exit) = handle_key_press(None, "Ctrl-C");
        assert!(!exit && armed.pending);
        let (_, exit2) = handle_key_press(armed.key_name, "Ctrl-C");
        assert!(exit2);
        // Different key resets the armed state.
        let (reset, exit3) = handle_key_press(Some("Ctrl-C"), "Ctrl-D");
        assert!(!exit3 && reset.key_name == Some("Ctrl-D"));
    }
}
