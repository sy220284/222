//! Rust exports derived from original: src/commands/review/ultrareviewEnabled.ts
//!
//! The original gates /ultrareview behind a vendor rollout config whose
//! `enabled` field controls visibility. With no rollout cache in this build
//! the config reads null, so `cfg?.enabled === true` is false — the default
//! path is reproduced exactly (command stays hidden until a gate source
//! exists).

/// `isUltrareviewEnabled` — null config yields false, byte-identical to the
/// original's ungranted rollout path.
pub fn is_ultrareview_enabled() -> bool {
    false
}
