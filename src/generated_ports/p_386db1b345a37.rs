//! Rust exports derived from original: src/utils/model/check1mAccess.ts
//!
//! Full equivalence: extra usage is enabled when there is no disabled reason
//! (null) or the reason indicates provisioning with depleted credits; an
//! absent cache reads conservative (false). All eight disabled reasons are
//! enumerated exactly as the original switch.

/// `OverageDisabledReason` — cached API answer variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverageDisabledReason {
    OutOfCredits,
    OverageNotProvisioned,
    OrgLevelDisabled,
    OrgLevelDisabledUntil,
    SeatTierLevelDisabled,
    MemberLevelDisabled,
    SeatTierZeroCreditLimit,
    GroupZeroCreditLimit,
    MemberZeroCreditLimit,
}

/// `isExtraUsageEnabled` over the cached reason.
pub fn is_extra_usage_enabled(reason: Option<Option<OverageDisabledReason>>) -> bool {
    match reason {
        // No cache yet — conservative.
        None => false,
        // null = no disabled reason from API.
        Some(None) => true,
        Some(Some(r)) => matches!(r, OverageDisabledReason::OutOfCredits),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use OverageDisabledReason::*;

    #[test]
    fn all_nine_states() {
        assert!(!is_extra_usage_enabled(None));
        assert!(is_extra_usage_enabled(Some(None)));
        assert!(is_extra_usage_enabled(Some(Some(OutOfCredits))));
        for r in [
            OverageNotProvisioned,
            OrgLevelDisabled,
            OrgLevelDisabledUntil,
            SeatTierLevelDisabled,
            MemberLevelDisabled,
            SeatTierZeroCreditLimit,
            GroupZeroCreditLimit,
            MemberZeroCreditLimit,
        ] {
            assert!(!is_extra_usage_enabled(Some(Some(r))));
        }
    }
}
