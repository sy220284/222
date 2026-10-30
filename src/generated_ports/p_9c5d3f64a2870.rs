//! Rust exports derived from original: src/migrations/migrateOpusToOpus1m.ts
//!
//! Migration contract preserved: migrate a pinned `opus` user setting to the
//! merged 1M alias when eligible. Runtime --model overrides are untouched
//! (not userSettings). Pro subscribers and 3P full-ID users are skipped.
//! Idempotent: writes only when userSettings.model is exactly the alias.
//! Model accessors port with the model chain; wiring lands there.

/// Decision for the migration given eligibility and the current setting:
/// write only when eligible AND the setting is exactly `opus`.
pub fn migration_decision(eligible: bool, current_setting: Option<&str>) -> bool {
    eligible && current_setting == Some("opus")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_exact_alias_is_migrated() {
        assert!(migration_decision(true, Some("opus")));
        assert!(!migration_decision(false, Some("opus")));
        assert!(!migration_decision(true, Some("opusplan")));
        assert!(!migration_decision(true, None));
    }
}
