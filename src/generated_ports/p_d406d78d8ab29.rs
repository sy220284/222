//! Rust exports derived from original: src/ink/components/AppContext.ts
//!
//! `AppContext` — React context exposing a manual exit (unmount) handle for
//! the whole Ink app, with a no-op default and displayName
//! "InternalAppContext". Owned by the ink runtime host.

/// Context identity note (original displayName).
pub const APP_CONTEXT_DISPLAY_NAME: &str = "InternalAppContext";
