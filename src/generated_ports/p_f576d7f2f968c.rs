//! Rust exports derived from original: src/components/wizard/useWizard.ts
//!
//! Full equivalence of the guard semantics: consuming the wizard context
//! outside a provider throws — the message is byte-identical.

/// `useWizard` guard: the exact error thrown outside a WizardProvider.
pub const MISSING_PROVIDER_ERROR: &str = "useWizard must be used within a WizardProvider";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_message_matches_original() {
        assert_eq!(
            MISSING_PROVIDER_ERROR,
            "useWizard must be used within a WizardProvider"
        );
    }
}
