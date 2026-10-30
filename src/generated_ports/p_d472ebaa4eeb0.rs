//! Rust exports derived from original: src/migrations/migrateLegacyOpusToCurrent.ts
//!
//! Full equivalence of the eligibility decision: first-party users still
//! pinned to explicit Opus 4.0/4.1 strings get migrated off them (the
//! "opus" alias already resolves to 4.6) and a one-time notification
//! timestamp is set. Gate combination mirrors the original gates.

/// Whether the legacy-string migration runs for this user.
pub fn migration_eligible(
    legacy_remap_enabled: bool,
    api_provider: &str,
    saved_model: Option<&str>,
) -> bool {
    let is_legacy = matches!(
        saved_model,
        Some("vendor-opus-4-0")
            | Some("vendor-opus-4-1")
            | Some("vendor-opus-4-0-20250514")
            | Some("vendor-opus-4-1-20250805")
    );
    legacy_remap_enabled && api_provider == "firstParty" && is_legacy
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_combination() {
        assert!(migration_eligible(
            true,
            "firstParty",
            Some("vendor-opus-4-1")
        ));
        assert!(!migration_eligible(
            false,
            "firstParty",
            Some("vendor-opus-4-1")
        ));
        assert!(!migration_eligible(
            true,
            "bedrock",
            Some("vendor-opus-4-1")
        ));
        assert!(!migration_eligible(true, "firstParty", Some("opus")));
    }
}
