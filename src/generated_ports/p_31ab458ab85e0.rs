//! Native Rust port derived from original: src/commands/passes/passes.tsx
//!
//! /passes entry: first visit marks `hasVisitedPasses` in global config
//! (stops the upsell) and snapshots `passesLastSeenRemaining` from the
//! referral cache; then renders the Passes view. Referral cache and
//! config persistence are injected as plain inputs.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde_json::{json, Value};

use crate::generated_ports::ui::VNode;

#[derive(Debug, Clone, PartialEq)]
pub struct PassesConfigPatch {
    pub has_visited_passes: bool,
    pub passes_last_seen_remaining: Option<u64>,
}

/// First-visit config patch (original inline logic): on first visit,
/// snapshot the cached remaining passes (None keeps the prior value).
pub fn first_visit_patch(
    has_visited: bool,
    cached_remaining: Option<u64>,
    prior_last_seen: Option<u64>,
) -> Option<PassesConfigPatch> {
    if has_visited {
        return None;
    }
    Some(PassesConfigPatch {
        has_visited_passes: true,
        passes_last_seen_remaining: cached_remaining.or(prior_last_seen),
    })
}

pub fn call(config: &Value, cached_remaining: Option<u64>) -> VNode {
    let has_visited = config
        .get("hasVisitedPasses")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let _ = first_visit_patch(
        has_visited,
        cached_remaining,
        config
            .get("passesLastSeenRemaining")
            .and_then(|v| v.as_u64()),
    );
    VNode::Element {
        tag: "Passes".into(),
        attrs: [("onDone".into(), json!("onDone"))].into_iter().collect(),
        children: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_visit_patches_config() {
        let patch = first_visit_patch(false, Some(3), None).unwrap();
        assert!(patch.has_visited_passes);
        assert_eq!(patch.passes_last_seen_remaining, Some(3));
        // Cache miss keeps prior value.
        let patch2 = first_visit_patch(false, None, Some(7)).unwrap();
        assert_eq!(patch2.passes_last_seen_remaining, Some(7));
    }

    #[test]
    fn repeat_visit_is_noop() {
        assert!(first_visit_patch(true, Some(3), None).is_none());
    }

    #[test]
    fn call_routes_into_passes_view() {
        let view = call(&json!({"hasVisitedPasses": true}), None);
        match view {
            VNode::Element { tag, .. } => assert_eq!(tag, "Passes"),
            _ => panic!(),
        }
    }
}
