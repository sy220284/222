//! Rust exports derived from original: src/utils/billing.ts
//!
//! `hasConsoleBillingAccess` decision chain: DISABLE_COST_WARNINGS kills it;
//! subscribers never see it; logged-out users don't see it; otherwise any
//! auth source grants access. Subscriber/auth predicates ride the auth port.

/// Whether the console may render cost data.
pub fn has_console_billing_access(
    cost_warnings_disabled: bool,
    is_subscriber: bool,
    auth_source_present: bool,
    api_key_present: bool,
) -> bool {
    if cost_warnings_disabled {
        return false;
    }
    if is_subscriber {
        return false;
    }
    auth_source_present || api_key_present
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decision_chain() {
        assert!(!has_console_billing_access(true, false, true, true));
        assert!(!has_console_billing_access(false, true, true, true));
        assert!(!has_console_billing_access(false, false, false, false));
        assert!(has_console_billing_access(false, false, true, false));
        assert!(has_console_billing_access(false, false, false, true));
    }
}
