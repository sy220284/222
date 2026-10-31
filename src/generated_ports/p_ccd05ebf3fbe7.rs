//! Rust exports derived from original: src/hooks/useDeferredHookMessages.ts
//!
//! Deferred SessionStart hook messages: the REPL renders immediately while
//! hook execution (~500ms) resolves in the background; onSubmit must flush
//! pending hook messages before the first API request so the model always
//! sees hook context. State machine ported; React timers ride the host.

/// Whether pending hook messages must be flushed before the first request.
pub fn needs_flush(resolved: bool) -> bool {
    !resolved
}

/// Transition after the background promise resolves: the pending slot is
/// always cleared regardless of whether the resolution carried messages or
/// was cancelled — the hook will not re-arm for the same turn (idempotent).
pub fn on_resolved(resolved: bool) -> bool {
    let _ = resolved;
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latch_semantics() {
        assert!(needs_flush(false));
        assert!(!needs_flush(true));
        assert!(on_resolved(false)); // first resolve flips the latch
        assert!(on_resolved(true)); // idempotent
    }
}
