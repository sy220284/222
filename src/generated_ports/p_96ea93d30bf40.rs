//! Rust exports derived from original: src/components/ink/AlternateScreen.tsx
//!
//! Props guard: children render only when the terminal supports the
//! alternate screen buffer; the enter/leave escape sequences belong to the
//! ink runtime host.

/// Whether children should render in this terminal.
pub fn renders_in_terminal(supports_alternate_screen: bool, force: bool) -> bool {
    supports_alternate_screen || force
}
