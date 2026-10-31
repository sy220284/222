//! Rust exports derived from original: src/hooks/useSettingsChange.ts
//!
//! Subscription contract: onChange(source, settings) fires per settings
//! source change. The original deliberately re-reads settings inside the
//! handler because the notifier already reset the cache (resetting here
//! caused N-way thrashing with N subscribers) — that comment is behavioral
//! documentation for the settings detector port.

/// Whether the handler should re-read settings itself: yes — the notifier
/// already reset the cache, so callers must not reset it again.
pub const HANDLER_MUST_NOT_RESET_CACHE: bool = true;
