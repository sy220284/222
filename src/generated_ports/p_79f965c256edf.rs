//! Native Rust port derived from original: src/hooks/toolPermission/handlers/coordinatorHandler.ts
//!
//! Coordinator worker permission flow: automated checks run sequentially
//! (hooks first — fast/local; then the bash classifier — slow/inference)
//! and any resolved decision is returned; otherwise null falls through
//! to the interactive dialog. Unexpected automated-check failures log
//! with a context prefix (non-Error throws keep the prefix; toError
//! would drop it) and also fall through. The context hooks/classifier
//! entry points are injected.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum PermissionDecision {
    Allowed,
    Denied(String),
}

#[derive(Debug, Clone, Default)]
pub struct CoordinatorPermissionParams<'a> {
    pub permission_mode: Option<&'a str>,
    pub suggestions: Option<&'a [Value]>,
    pub updated_input: Option<&'a Value>,
    pub pending_classifier_check: Option<&'a Value>,
}

#[derive(Default)]
pub struct CheckOutcome {
    pub hook_result: Option<PermissionDecision>,
    pub classifier_result: Option<PermissionDecision>,
    pub check_error: Option<String>,
}

/// `handleCoordinatorPermission` decision core: hook → classifier →
/// dialog fallthrough, with the error fallthrough preserved.
pub fn handle_coordinator_permission(
    params: &CoordinatorPermissionParams<'_>,
    outcome: &CheckOutcome,
) -> Option<PermissionDecision> {
    // 1. Hooks first (fast, local).
    if let Some(decision) = &outcome.hook_result {
        return Some(decision.clone());
    }
    // 2. Classifier (slow, inference — bash only).
    if let Some(decision) = &outcome.classifier_result {
        return Some(decision.clone());
    }
    // Errors and unresolved checks fall through to the interactive dialog.
    if let Some(err) = &outcome.check_error {
        // Traceable log line (original prefixes non-Error throws).
        log_line(&format!(
            "Automated permission check failed: {err}"
        ));
    }
    None
}

fn log_line(message: &str) {
    // Original logError; kept as a call-shape hook for the log port.
    let _ = message;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_resolves_before_classifier() {
        let params = CoordinatorPermissionParams::default();
        let outcome = CheckOutcome {
            hook_result: Some(PermissionDecision::Allowed),
            classifier_result: Some(PermissionDecision::Denied("c".into())),
            ..Default::default()
        };
        assert_eq!(
            handle_coordinator_permission(&params, &outcome),
            Some(PermissionDecision::Allowed)
        );
    }

    #[test]
    fn classifier_resolves_when_hook_null() {
        let params = CoordinatorPermissionParams::default();
        let outcome = CheckOutcome {
            classifier_result: Some(PermissionDecision::Denied("bash rm".into())),
            ..Default::default()
        };
        assert_eq!(
            handle_coordinator_permission(&params, &outcome),
            Some(PermissionDecision::Denied("bash rm".into()))
        );
    }

    #[test]
    fn errors_fall_through_to_dialog() {
        let params = CoordinatorPermissionParams::default();
        let outcome = CheckOutcome {
            check_error: Some("boom".into()),
            ..Default::default()
        };
        assert_eq!(handle_coordinator_permission(&params, &outcome), None);
    }

    #[test]
    fn unresolved_checks_fall_through() {
        let params = CoordinatorPermissionParams::default();
        assert_eq!(
            handle_coordinator_permission(&params, &CheckOutcome::default()),
            None
        );
    }
}
