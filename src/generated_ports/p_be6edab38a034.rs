//! Rust exports derived from original: src/hooks/notifs/useNpmDeprecationNotification.tsx
//!
//! Notification payload contract. The original text names the vendor product
//! and docs host; per the privacy gate (brand-neutral source) the message is
//! recorded in its neutralized form, with the guard chain reproduced exactly:
//! bundled mode or DISABLE_INSTALLATION_CHECKS suppresses it, development
//! installs are skipped, and the payload is timeout 15000 / warning / high.

/// The neutralized deprecation notice.
pub const NPM_DEPRECATION_MESSAGE: &str =
    "The CLI has switched from npm to the native installer. Run the native install command or see the documentation for more options.";

/// Startup-notification payload (mirrors the original object shape).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct StartupNotification {
    pub timeout_ms: u64,
    pub key: &'static str,
    pub text: &'static str,
    pub color: &'static str,
    pub priority: &'static str,
}

/// `_temp` — the guarded payload construction (full decision chain).
pub fn npm_deprecation_notification(
    in_bundled_mode: bool,
    installation_checks_disabled: bool,
    installation_type: Option<&str>,
) -> Option<StartupNotification> {
    if in_bundled_mode || installation_checks_disabled {
        return None;
    }
    if installation_type == Some("development") {
        return None;
    }
    Some(StartupNotification {
        timeout_ms: 15000,
        key: "npm-deprecation-warning",
        text: NPM_DEPRECATION_MESSAGE,
        color: "warning",
        priority: "high",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guards_match_original_chain() {
        assert!(npm_deprecation_notification(true, false, None).is_none());
        assert!(npm_deprecation_notification(false, true, None).is_none());
        assert!(npm_deprecation_notification(false, false, Some("development")).is_none());
        let notice = npm_deprecation_notification(false, false, Some("global")).unwrap();
        assert_eq!(notice.timeout_ms, 15000);
        assert_eq!(notice.priority, "high");
    }
}
