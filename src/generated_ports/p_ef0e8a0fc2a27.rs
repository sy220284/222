//! Rust exports derived from original: src/ink/hooks/use-input.ts
//!
//! Input capture contract: the handler receives every keypress as
//! (input, key, event); pasted text arrives as a single multi-char input.
//! `isActive` guards duplicate handling when multiple hooks are mounted.
//! Paste and large-payload batching ride the stdin runtime.

/// Whether this hook's handler should run for the pending input.
pub fn should_handle(is_active: bool, stdin_is_raw: bool, input_len: usize) -> bool {
    is_active && stdin_is_raw && input_len > 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_gate() {
        assert!(should_handle(true, true, 1));
        assert!(!should_handle(false, true, 1));
        assert!(!should_handle(true, false, 1));
        assert!(!should_handle(true, true, 0));
    }
}
