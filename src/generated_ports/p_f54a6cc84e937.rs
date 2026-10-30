//! Rust exports derived from original: src/hooks/useVoiceEnabled.ts
//!
//! Composition contract: user intent (settings.voiceEnabled) AND voice auth
//! AND the growthbook kill-switch. Only the auth half is memoized in the
//! original (expensive OAuth token read); the kill-switch stays outside the
//! memo so a mid-session flip still applies.

/// The combined decision over the three inputs.
pub fn voice_enabled(user_intent: bool, voice_authed: bool, growthbook_enabled: bool) -> bool {
    user_intent && voice_authed && growthbook_enabled
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_three_inputs_required() {
        assert!(!voice_enabled(true, true, false));
        assert!(!voice_enabled(true, false, true));
        assert!(!voice_enabled(false, true, true));
        assert!(voice_enabled(true, true, true));
    }
}
