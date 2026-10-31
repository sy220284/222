//! Rust exports derived from original: src/utils/computerUse/escHotkey.ts
//!
//! Global Escape → abort via a CGEventTap (macOS-only native module): while
//! registered Escape is consumed system-wide as a prompt-injection defense.
//! Lifecycle: register on fresh lock acquire, unregister on release; a
//! refcounted runloop-pump retain spans the registration; the executor
//! punches a 100ms decay hole for model-synthesized Escapes. The native tap
//! itself is unavailable in Rust (no @ant/computer-use-swift); the state
//! machine and error semantics are the port.

/// `registerEscHotkey` — idempotent registration; failure mirrors a missing
/// Accessibility permission (CU still works, just without ESC abort).
pub fn register_esc_hotkey(registered: bool, tap_created: bool) -> Result<bool, &'static str> {
    if registered {
        return Ok(true);
    }
    if tap_created {
        Ok(true)
    } else {
        Err("CGEvent.tapCreate failed (missing Accessibility permission)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idempotent_and_failure_path() {
        assert_eq!(register_esc_hotkey(true, false), Ok(true));
        assert!(register_esc_hotkey(false, false).is_err());
        assert_eq!(register_esc_hotkey(false, true), Ok(true));
    }
}
