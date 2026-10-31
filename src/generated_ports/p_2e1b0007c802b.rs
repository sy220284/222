//! Rust exports derived from original: src/migrations/migrateSonnet1mToSonnet45.ts
//!
//! Full equivalence of the alias migration decision: users who saved the
//! bare "sonnet]" alias (targeting Sonnet 4.5 with 1M context) get pinned
//! to the explicit "sonnet-4-5-20250929]" because "sonnet" now resolves to
//! Sonnet 4.6. Reads userSettings only so project-scoped aliases are not
//! promoted globally; idempotent.

/// The explicit pinned model written by the migration.
pub const PINNED_SONNET_1M: &str = "sonnet-4-5-20250929]";

/// `migrate` decision — Some(pinned) when the saved model is the legacy
/// bare alias; None leaves settings untouched.
pub fn migrate_decision(saved_model: Option<&str>) -> Option<&'static str> {
    match saved_model {
        Some("sonnet]") => Some(PINNED_SONNET_1M),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_bare_sonnet_alias_pinned() {
        assert_eq!(migrate_decision(Some("sonnet]")), Some(PINNED_SONNET_1M));
        assert_eq!(migrate_decision(Some("sonnet")), None);
        assert_eq!(migrate_decision(Some(PINNED_SONNET_1M)), None);
        assert_eq!(migrate_decision(None), None);
    }
}
