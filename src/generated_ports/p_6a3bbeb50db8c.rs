//! Rust exports derived from original: src/hooks/useSettings.ts
//!
//! Reactive settings accessor: the original reads AppState.settings, which
//! updates automatically when files change on disk via the settings change
//! detector. The Rust app-state host owns the subscription; the contract is
//! recorded here.

/// `ReadonlySettings` — the AppState settings slice (DeepImmutable in the
/// original); resolves to the crate settings type through the app-state host.
pub const READONLY_SETTINGS_SLICE: &str = "AppState.settings";
