//! Native Rust port derived from original: src/commands/rate-limit-options/rate-limit-options.tsx
//!
//! Rate-limit options menu decision core: subscription/tier gates
//! (max, max-20x, team/enterprise), the extra-usage option with its
//! admin-request vs self-serve labels and the org spend-cap exclusion,
//! and the upgrade option excluded for max-20x/team/enterprise. The
//! experiment flag name is elided for the privacy scan; Select/Dialog
//! rendering is component-domain.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuInputs {
    pub subscription_max: bool,
    pub tier_max_20x: bool,
    pub team_or_enterprise: bool,
    pub has_extra_usage_enabled: bool,
    pub has_billing_access: bool,
    pub extra_usage_enabled: bool,
    pub upgrade_enabled: bool,
    pub buy_first: bool,
    pub overage_disabled_reason: Option<&'static str>,
    pub overage_status: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuOption {
    pub label: String,
    pub value: &'static str,
}

fn is_org_spend_cap_depleted(reason: Option<&str>) -> bool {
    matches!(
        reason,
        Some("out_of_credits")
            | Some("org_level_disabled_until")
            | Some("org_service_zero_credit_limit")
    )
}

fn is_overage_state(status: Option<&str>) -> bool {
    matches!(status, Some("rejected") | Some("allowed_warning"))
}

/// Extra-usage option: hidden when an admin must request and the org
/// spend cap is depleted; labels differ by admin-request and enablement.
pub fn extra_usage_option(inputs: &MenuInputs) -> Option<MenuOption> {
    if !inputs.extra_usage_enabled {
        return None;
    }
    let needs_admin = inputs.team_or_enterprise && !inputs.has_billing_access;
    if needs_admin && is_org_spend_cap_depleted(inputs.overage_disabled_reason) {
        return None;
    }
    let label = if needs_admin {
        if is_overage_state(inputs.overage_status) {
            "Request more"
        } else {
            "Request extra usage"
        }
    } else if inputs.has_extra_usage_enabled {
        "Add funds to continue with extra usage"
    } else {
        "Switch to extra usage"
    };
    Some(MenuOption {
        label: label.into(),
        value: "extra-usage",
    })
}

/// Upgrade option: excluded for max-20x and team/enterprise.
pub fn upgrade_option(inputs: &MenuInputs) -> Option<MenuOption> {
    if inputs.tier_max_20x || inputs.team_or_enterprise || !inputs.upgrade_enabled {
        return None;
    }
    Some(MenuOption {
        label: "Upgrade your plan".into(),
        value: "upgrade",
    })
}

/// Full action list in original order, plus the cancel option.
pub fn action_options(inputs: &MenuInputs) -> Vec<MenuOption> {
    let mut options = Vec::new();
    if let Some(opt) = extra_usage_option(inputs) {
        options.push(opt);
    }
    if let Some(opt) = upgrade_option(inputs) {
        options.push(opt);
    }
    options.push(MenuOption {
        label: "Cancel".into(),
        value: "cancel",
    });
    options
}

/// Registry view shim.
pub fn rateLimitMenuLabels(_args: &[Value]) -> Value {
    json!([])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> MenuInputs {
        MenuInputs {
            subscription_max: false,
            tier_max_20x: false,
            team_or_enterprise: false,
            has_extra_usage_enabled: false,
            has_billing_access: true,
            extra_usage_enabled: true,
            upgrade_enabled: true,
            buy_first: false,
            overage_disabled_reason: None,
            overage_status: None,
        }
    }

    #[test]
    fn extra_usage_labels_by_admin_and_enablement() {
        let mut i = base();
        assert_eq!(
            extra_usage_option(&i).unwrap().label,
            "Switch to extra usage"
        );
        i.has_extra_usage_enabled = true;
        assert_eq!(
            extra_usage_option(&i).unwrap().label,
            "Add funds to continue with extra usage"
        );
        i.team_or_enterprise = true;
        i.has_billing_access = false;
        assert_eq!(extra_usage_option(&i).unwrap().label, "Request extra usage");
        i.overage_status = Some("rejected");
        assert_eq!(extra_usage_option(&i).unwrap().label, "Request more");
    }

    #[test]
    fn org_spend_cap_depleted_hides_admin_request() {
        let mut i = base();
        i.team_or_enterprise = true;
        i.has_billing_access = false;
        i.overage_disabled_reason = Some("out_of_credits");
        assert!(extra_usage_option(&i).is_none());
        i.has_billing_access = true;
        assert!(extra_usage_option(&i).is_some());
    }

    #[test]
    fn upgrade_excluded_for_max20x_and_teams() {
        let mut i = base();
        assert_eq!(upgrade_option(&i).unwrap().value, "upgrade");
        i.tier_max_20x = true;
        assert!(upgrade_option(&i).is_none());
        let mut j = base();
        j.team_or_enterprise = true;
        assert!(upgrade_option(&j).is_none());
    }

    #[test]
    fn full_order_and_cancel_tail() {
        let options = action_options(&base());
        assert_eq!(options.len(), 3);
        assert_eq!(options[0].value, "extra-usage");
        assert_eq!(options[1].value, "upgrade");
        assert_eq!(options.last().unwrap().value, "cancel");
    }
}
