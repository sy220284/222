//! Rust exports derived from original: src/components/LogoV2/GuestPassesUpsell.tsx
//!
//! Upsell decision core. `resetIfPassesRefreshed` semantics: a non-null,
//! positive remaining count that exceeds the last-seen value resets the
//! upsell counters and records the new total. `shouldShowGuestPassesUpsell`
//! requires eligibility plus a warm cache (never blocks on fetch).

#[derive(Debug, Clone, Default)]
pub struct PassesUpsellReset {
    pub passes_upsell_seen_count: u32,
    pub has_visited_passes: bool,
    pub passes_last_seen_remaining: u32,
}

/// `resetIfPassesRefreshed` decision: returns Some(new config fields) when
/// passes were refreshed since last seen.
pub fn reset_if_passes_refreshed(
    remaining: Option<u32>,
    last_seen: u32,
) -> Option<PassesUpsellReset> {
    let remaining = remaining?;
    if remaining == 0 || remaining <= last_seen {
        return None;
    }
    Some(PassesUpsellReset {
        passes_upsell_seen_count: 0,
        has_visited_passes: false,
        passes_last_seen_remaining: remaining,
    })
}

/// `shouldShowGuestPassesUpsell` — eligible and cache must both hold.
pub fn should_show_upsell(eligible: bool, has_cache: bool) -> bool {
    eligible && has_cache
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_reset_semantics() {
        assert!(reset_if_passes_refreshed(Some(5), 2).is_some());
        assert!(reset_if_passes_refreshed(Some(2), 5).is_none());
        assert!(reset_if_passes_refreshed(Some(0), 0).is_none());
        assert!(reset_if_passes_refreshed(None, 0).is_none());
    }

    #[test]
    fn upsell_gate() {
        assert!(should_show_upsell(true, true));
        assert!(!should_show_upsell(true, false));
        assert!(!should_show_upsell(false, true));
    }
}
