//! Rust exports derived from original: src/utils/plugins/performStartupChecks.tsx
//!
//! Trust gate for plugin startup checks: background installations only run
//! after the "trust this folder" dialog has been accepted — the trust dialog
//! blocks all execution until the user explicitly trusts the cwd, so
//! malicious repositories can never self-install without user approval.

/// Whether startup plugin checks may run for this directory.
pub fn startup_checks_allowed(trust_dialog_accepted: bool) -> bool {
    trust_dialog_accepted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trust_gate() {
        assert!(startup_checks_allowed(true));
        assert!(!startup_checks_allowed(false));
    }
}
