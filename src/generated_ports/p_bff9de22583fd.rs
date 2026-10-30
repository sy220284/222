//! Rust exports derived from original:
//! src/components/PromptInput/useMaybeTruncateInput.ts
//!
//! One-shot truncation gate: runs `maybeTruncateInput` only once per distinct
//! input value (10,000-char threshold, 20,000 hard cap), mutating input,
//! cursor and pasted contents via the callbacks; the applied-marker never
//! resets for the lifetime of the component instance.

/// Whether truncation processing should run for this input now.
pub fn should_process(has_applied: bool, input_len: usize) -> bool {
    !has_applied && input_len > 10_000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_semantics() {
        assert!(!should_process(false, 9_999));
        assert!(should_process(false, 10_001));
        assert!(!should_process(true, 20_000));
    }
}
