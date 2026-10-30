//! Rust exports derived from original:
//! src/components/messages/AssistantRedactedThinkingMessage.tsx
//!
//! JSX view structure: dim italic "✻ Thinking…" with optional top margin
//! (addMargin defaults false).

/// Text content of the view.
pub const THINKING_TEXT: &str = "✻ Thinking…";

/// marginTop value for the outer box (0 or 1).
pub fn margin_top(add_margin: Option<bool>) -> u8 {
    u8::from(add_margin.unwrap_or(false))
}
