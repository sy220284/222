//! Rust exports derived from original: src/ink/components/TerminalFocusContext.tsx
//!
//! Terminal focus view-model: isTerminalFocused + focus state string,
//! defaults focused=true / state="unknown". The provider subscribes to the
//! terminal-focus-state store (get/subscribe triple) so only context
//! consumers re-render — the React context instance belongs to the ink
//! runtime host.

pub const FOCUS_DEFAULT: &str = "unknown";
pub const FOCUS_CONTEXT_DISPLAY_NAME: &str = "TerminalFocusContext";
