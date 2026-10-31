//! Rust exports derived from original: src/utils/backgroundHousekeeping.ts
//!
//! Interval constants and the housekeeping roster. Compile-time gated
//! features (EXTRACT_MEMORIES / LODESTONE) read closed in external builds;
//! the roster below records the scheduling shape for the runtime host.

/// Recurring cleanup cadence (24 hours).
pub const RECURRING_CLEANUP_INTERVAL_MS: u64 = 24 * 60 * 60 * 1000;
/// Very slow one-shot operations start 10 minutes after launch.
pub const DELAY_VERY_SLOW_OPERATIONS_MS: u64 = 10 * 60 * 1000;
/// Fast recurring housekeeping cadence (5 minutes).
pub const FAST_RECURRING_INTERVAL_MS: u64 = 5 * 60 * 1000;

/// Roster entries whose init functions port with their own feature chains.
/// Vendor NPM cache cleanup is excluded from the independent Rust runtime.
pub const HOUSEKEEPING_ROSTER: &[&str] = &[
    "cleanupOldMessageFilesInBackground",
    "cleanupOldVersionsThrottled",
    "cleanupOldVersions",
    "autoUpdateMarketplacesAndPluginsInBackground",
    "initAutoDream",
    "initMagicDocs",
    "initSkillImprovement",
    "initExtractMemories(gate-closed)",
    "registerDeepLinkProtocol(gate-closed)",
];
