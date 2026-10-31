//! Rust exports derived from original: src/migrations/migrateBypassPermissionsAcceptedToSettings.ts
//!
//! Migration contract preserved: move bypassPermissionsModeAccepted from the
//! global config into settings.json as skipDangerousModePermissionPrompt
//! (unless already set), then drop the old key — idempotent by the early
//! return. The settings/config accessors port with their own files; wiring
//! lands when the settings runtime chain is accepted.

/// Decision for the migration given the config flag and the settings state:
/// (should_write_setting, should_drop_config_key). The original performs
/// the write, logs the vendor analytics event (excluded here) and saves.
pub fn migration_decision(config_flag_set: bool, settings_already_has: bool) -> (bool, bool) {
    if !config_flag_set {
        return (false, false);
    }
    (!settings_already_has, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idempotent_and_write_only_when_needed() {
        assert_eq!(migration_decision(false, false), (false, false));
        assert_eq!(migration_decision(true, false), (true, true));
        assert_eq!(migration_decision(true, true), (false, true));
    }
}
