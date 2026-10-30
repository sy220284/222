//! Rust exports derived from original: src/hooks/useMainLoopModel.ts
//!
//! The hook subscribes to AppState mainLoopModel/mainLoopModelForSession and
//! re-resolves on GrowthBook refresh (stale-cache to fresh remoteEval). The
//! resolver (parseUserSpecifiedModel) ports with the model chain; this stub
//! records the subscription contract pending the app-state host.

/// Re-resolution triggers for the main loop model display.
pub const RE_RESOLVE_ON_GROWTHBOOK_REFRESH: &str = "onGrowthBookRefresh";
