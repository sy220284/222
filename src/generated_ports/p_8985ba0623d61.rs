//! Native Rust port derived from original: src/services/remoteManagedSettings/securityCheck.tsx
//!
//! Managed-settings security gate: three no-check-needed short-circuits
//! (no dangerous settings / dangerous settings unchanged / non-interactive)
//! then a blocking approval dialog. The dangerous-settings extraction and
//! diff predicates are injected as booleans (their port lives with the
//! ManagedSettingsSecurityDialog utils); the dialog itself is
//! component-domain. Telemetry events kept as call-shape comments.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityCheckResult {
    Approved,
    Rejected,
    NoCheckNeeded,
}

/// `checkManagedSettingsSecurity` decision core over injected predicates:
/// `new_has_dangerous` = hasDangerousSettings(extractDangerousSettings(new)),
/// `dangerous_changed` = hasDangerousSettingsChanged(cached, new),
/// `is_interactive` from the bootstrap state.
pub fn check_managed_settings_security(
    new_has_dangerous: bool,
    dangerous_changed: bool,
    is_interactive: bool,
) -> SecurityCheckDecision {
    if !new_has_dangerous {
        return SecurityCheckDecision::Resolved(SecurityCheckResult::NoCheckNeeded);
    }
    if !dangerous_changed {
        return SecurityCheckDecision::Resolved(SecurityCheckResult::NoCheckNeeded);
    }
    if !is_interactive {
        return SecurityCheckDecision::Resolved(SecurityCheckResult::NoCheckNeeded);
    }
    // Dialog shown (telemetry: managed_settings_security_dialog_shown);
    // user acceptance/rejection resolves approved/rejected.
    SecurityCheckDecision::BlockingDialog
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityCheckDecision {
    Resolved(SecurityCheckResult),
    BlockingDialog,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_dangerous_settings_short_circuits() {
        assert_eq!(
            check_managed_settings_security(false, false, true),
            SecurityCheckDecision::Resolved(SecurityCheckResult::NoCheckNeeded)
        );
    }

    #[test]
    fn unchanged_dangerous_short_circuits() {
        assert_eq!(
            check_managed_settings_security(true, false, true),
            SecurityCheckDecision::Resolved(SecurityCheckResult::NoCheckNeeded)
        );
    }

    #[test]
    fn non_interactive_skips_dialog() {
        assert_eq!(
            check_managed_settings_security(true, true, false),
            SecurityCheckDecision::Resolved(SecurityCheckResult::NoCheckNeeded)
        );
    }

    #[test]
    fn changed_dangerous_interactive_blocks() {
        assert_eq!(
            check_managed_settings_security(true, true, true),
            SecurityCheckDecision::BlockingDialog
        );
    }
}
