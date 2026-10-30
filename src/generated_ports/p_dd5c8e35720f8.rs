//! Rust exports derived from original: src/ink/components/TerminalSizeContext.tsx
//!
//! Terminal size view-model: columns/rows pair, null until the terminal
//! reports a size. The React context instance itself is owned by the ink
//! runtime host.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TerminalSize {
    pub columns: u16,
    pub rows: u16,
}
