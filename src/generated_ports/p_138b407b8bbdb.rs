//! Rust exports derived from original:
//! src/migrations/resetAutoModeOptInForDefaultOffer.ts
//!
//! Full equivalence: one-shot migration clears skipAutoPermissionPrompt so
//! users of the old two-option dialog see the new "make it my default" offer.
//! Guards reproduced exactly: the compile-time classifier gate reads closed
//! in external builds (early return), and only 'enabled' auto-mode users are
//! eligible — for 'opt-in' users clearing the skip flag would remove auto
//! from the carousel and defeat the migration. Idempotent: only acts when
//! skipAutoPermissionPrompt is set.

/// `resetAutoModeOptInForDefaultOffer` decision core: does this user get the
/// flag cleared?
pub fn should_clear(
    classifier_feature_on: bool,
    auto_mode_state: &str,
    skip_prompt_set: bool,
) -> bool {
    classifier_feature_on && auto_mode_state == "enabled" && skip_prompt_set
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guards_match_original() {
        // External build: gate closed, never clears.
        assert!(!should_clear(false, "enabled", true));
        // opt-in users keep the flag (dialog would become unreachable).
        assert!(!should_clear(true, "opt-in", true));
        assert!(!should_clear(true, "enabled", false));
        assert!(should_clear(true, "enabled", true));
    }
}
