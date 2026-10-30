//! Rust exports derived from original: src/components/sandbox/SandboxConfigTab.tsx
//!
//! Config tab decision core: enabled check + dependency warnings surfaced as
//! a note block; disabled copy "Sandbox is not enabled". The sandbox adapter
//! (SandboxManager) ports with the sandbox chain; JSX belongs to the ink
//! host.

/// Whether the tab shows the disabled placeholder instead of live settings.
pub fn shows_disabled_placeholder(is_enabled: bool) -> bool {
    !is_enabled
}

/// Warnings block visibility: only when dependency check produced warnings.
pub fn shows_dep_warnings(warning_count: usize) -> bool {
    warning_count > 0
}
