//! Rust exports derived from original: src/migrations/resetProToOpusDefault.ts
//!
//! Migration decision core: once-per-install flag (opusProMigrationComplete)
//! guards everything; non-firstParty or non-Pro installs just mark complete
//! and skip; eligible Pro users migrate the default model to the current
//! Opus default. Subscriber/provider predicates ride the auth/model chain
//! (vendor subscription surface); logEvent is the excluded telemetry surface.

/// Whether the migration should migrate the model default (given guards).
pub fn should_migrate_model_default(api_provider: &str, is_pro_subscriber: bool) -> bool {
    api_provider == "firstParty" && is_pro_subscriber
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eligibility_gates() {
        assert!(should_migrate_model_default("firstParty", true));
        assert!(!should_migrate_model_default("firstParty", false));
        assert!(!should_migrate_model_default("bedrock", true));
    }
}
