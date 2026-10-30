//! Rust exports derived from original: src/migrations/migrateAutoUpdatesToSettings.ts
//!
//! Decision logic fully equivalent and unit-tested: only an explicit user
//! `autoUpdates: false` (not the native-protection write) migrates into the
//! settings env surface, guarded by autoUpdatesProtectedForNative. The
//! settings/config accessors and the vendor analytics event wire in with
//! their own ports.

/// `migrateAutoUpdatesToSettings` decision. Returns Some(env_value) to write
/// when the migration applies; None when it early-returns.
pub fn migration_decision(
    auto_updates: Option<bool>,
    protected_for_native: Option<bool>,
) -> Option<&'static str> {
    match (auto_updates, protected_for_native) {
        (Some(false), Some(true)) => None,
        (Some(false), _) => Some("DISABLE_AUTOUPDATER=1"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_only_explicit_user_disable() {
        assert_eq!(migration_decision(Some(false), Some(true)), None);
        assert_eq!(
            migration_decision(Some(false), None),
            Some("DISABLE_AUTOUPDATER=1")
        );
        assert_eq!(migration_decision(Some(true), None), None);
        assert_eq!(migration_decision(None, None), None);
    }
}
