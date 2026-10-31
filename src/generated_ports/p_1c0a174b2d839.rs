//! Rust exports derived from original:
//! src/hooks/notifs/useAutoModeUnavailableNotification.ts
//!
//! One-shot notification when the shift-tab carousel wraps past where auto
//! mode would have been, covering all reasons (settings, circuit-breaker,
//! org-allowlist). The startup downgrade case is handled elsewhere
//! (verifyAutoModeGateAccess → checkAndDisableAutoModeIfNeeded). The
//! compile-time auto-mode gate reads closed, matching the original's
//! external-build behavior; the reason/message helpers port with the
//! permission-setup chain.

/// Whether this hook can ever fire (compile gate + remote mode guard).
pub fn notification_eligible(auto_mode_feature_on: bool, is_remote_mode: bool) -> bool {
    auto_mode_feature_on && !is_remote_mode
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_build_never_fires() {
        assert!(!notification_eligible(false, false));
        assert!(!notification_eligible(true, true));
    }
}
