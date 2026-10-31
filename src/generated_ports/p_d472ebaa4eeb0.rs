//! Provider-neutral eligibility predicate derived from the legacy-model migration.
//!
//! Migration policy belongs to the configured provider: this module never
//! embeds a vendor model list or changes a user's selected model implicitly.
//! Settings persistence and one-time notifications remain separate obligations.

pub fn migration_eligible(
    remap_enabled: bool,
    api_provider: &str,
    migration_provider: &str,
    saved_model: Option<&str>,
    legacy_models: &[&str],
) -> bool {
    remap_enabled
        && api_provider == migration_provider
        && saved_model.is_some_and(|model| legacy_models.contains(&model))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_combination() {
        let models = [
            "legacy-4-0",
            "legacy-4-1",
            "legacy-4-20250514",
            "legacy-4-1-20250805",
        ];
        for model in models {
            assert!(migration_eligible(
                true,
                "local",
                "local",
                Some(model),
                &models
            ));
            assert!(!migration_eligible(
                false,
                "local",
                "local",
                Some(model),
                &models
            ));
            assert!(!migration_eligible(
                true,
                "other",
                "local",
                Some(model),
                &models
            ));
        }
        assert!(!migration_eligible(
            true,
            "local",
            "local",
            Some("current"),
            &models
        ));
        assert!(!migration_eligible(true, "local", "local", None, &models));
        assert!(!migration_eligible(
            true,
            "local",
            "local",
            Some("legacy-4-0"),
            &[]
        ));
    }
}
