//! Native Rust port derived from original: src/commands/upgrade/upgrade.tsx
//!
//! /upgrade orchestration: Max-20x detection via OAuth tokens
//! (subscriptionType+rateLimitTier) falling back to the profile lookup;
//! highest-plan message short-circuits; otherwise open the upgrade URL
//! and route into the Login view with the upstream starting message;
//! browser failure degrades to a visit-URL message.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde_json::{json, Value};

use crate::generated_ports::ui::VNode;

pub const UPGRADE_URL: &str = "https://upstream.example/upgrade/max";
pub const STARTING_MESSAGE: &str =
    "Starting new login following /upgrade. Exit with Ctrl-C to use existing account.";
pub const MAX_20X_MESSAGE: &str =
    "You are already on the highest Max subscription plan. For additional usage, run /login to switch to an API usage-billed account.";
pub const BROWSER_FAIL_MESSAGE: &str =
    "Failed to open browser. Please visit https://upstream.example/upgrade/max to upgrade.";

#[derive(Debug, Clone, Default)]
pub struct OauthTokens {
    pub subscription_type: Option<String>,
    pub rate_limit_tier: Option<String>,
    pub access_token: Option<String>,
}

/// Token-path Max-20x detection (original inline logic).
pub fn is_max_20x_from_tokens(tokens: &OauthTokens) -> Option<bool> {
    match (&tokens.subscription_type, &tokens.rate_limit_tier) {
        (Some(sub), Some(tier)) => Some(sub == "max" && tier == "default_vendor_max_tier_20x"),
        _ => None,
    }
}

/// Profile-path fallback (profile.organization check).
pub fn is_max_20x_from_profile(org_type: Option<&str>, rate_tier: Option<&str>) -> bool {
    org_type == Some("vendor_max_tier") && rate_tier == Some("default_vendor_max_tier_20x")
}

#[derive(Debug, Clone, PartialEq)]
pub enum UpgradeOutcome {
    AlreadyHighest,
    OpenBrowserAndLogin,
    BrowserFailed,
}

/// Branch orchestration over injected lookups.
pub fn orchestrate(
    is_subscriber: bool,
    token_check: Option<bool>,
    profile_check: bool,
    browser_ok: bool,
) -> UpgradeOutcome {
    if is_subscriber {
        let is_max = token_check.unwrap_or(profile_check);
        if is_max {
            return UpgradeOutcome::AlreadyHighest;
        }
    }
    if browser_ok {
        UpgradeOutcome::OpenBrowserAndLogin
    } else {
        UpgradeOutcome::BrowserFailed
    }
}

pub fn render_login_view() -> VNode {
    VNode::Element {
        tag: "Login".into(),
        attrs: [
            ("startingMessage".into(), json!(STARTING_MESSAGE)),
            ("onDone".into(), json!("onChangeAPIKey + onDone(success)")),
        ]
        .into_iter()
        .collect(),
        children: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_path_max20x_detection() {
        let t = OauthTokens {
            subscription_type: Some("max".into()),
            rate_limit_tier: Some("default_vendor_max_tier_20x".into()),
            access_token: None,
        };
        assert_eq!(is_max_20x_from_tokens(&t), Some(true));
        let t2 = OauthTokens {
            subscription_type: Some("max".into()),
            rate_limit_tier: Some("other_tier".into()),
            access_token: None,
        };
        assert_eq!(is_max_20x_from_tokens(&t2), Some(false));
        let t3 = OauthTokens {
            ..Default::default()
        };
        assert_eq!(is_max_20x_from_tokens(&t3), None);
    }

    #[test]
    fn profile_path_fallback() {
        assert!(is_max_20x_from_profile(
            Some("vendor_max_tier"),
            Some("default_vendor_max_tier_20x")
        ));
        assert!(!is_max_20x_from_profile(
            Some("vendor_pro"),
            Some("default_vendor_max_tier_20x")
        ));
    }

    #[test]
    fn orchestration_branches() {
        assert_eq!(
            orchestrate(true, Some(true), false, true),
            UpgradeOutcome::AlreadyHighest
        );
        // Token check None falls through to profile.
        assert_eq!(
            orchestrate(true, None, true, true),
            UpgradeOutcome::AlreadyHighest
        );
        assert_eq!(
            orchestrate(false, None, false, true),
            UpgradeOutcome::OpenBrowserAndLogin
        );
        assert_eq!(
            orchestrate(false, None, false, false),
            UpgradeOutcome::BrowserFailed
        );
    }

    #[test]
    fn login_view_carries_starting_message() {
        match render_login_view() {
            VNode::Element { attrs, .. } => assert!(attrs
                .iter()
                .any(|(k, v)| k == "startingMessage" && v.as_str().unwrap().contains("/upgrade"))),
            _ => panic!(),
        }
    }
}
