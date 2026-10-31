//! Rust exports derived from original: src/moreright/useMoreRight.tsx
//!
//! External-build stub — the real hook is internal only. The original is
//! self-contained and no-op: onBeforeQuery resolves true, onTurnComplete is
//! empty, render returns null. Reproduced exactly.

/// `useMoreRight` external-build stub semantics.
#[derive(Debug, Clone, Copy, Default)]
pub struct MoreRightStub;

impl MoreRightStub {
    pub fn on_before_query(&self) -> bool {
        true
    }
    pub fn on_turn_complete(&self) {}
    pub fn render(&self) {}
}
