//! Native Rust port derived from original: src/utils/agentContext.ts
//!
//! Agent context for analytics attribution. The original uses Node's
//! `AsyncLocalStorage` so concurrently-backgrounded agents (ctrl+b) never
//! cross-contaminate context; the Rust equivalent is `tokio::task_local!`,
//! which isolates per async task with the same guarantees.
//!
//! Two agent kinds: `subagent` (Agent tool, in-process) and `teammate`
//! (swarm, in-process). Swarm teammates in separate processes use env
//! vars upstream — same convention here (ATLAS_AGENT_ID /
//! ATLAS_PARENT_SESSION_ID), recorded in the port matrix.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use std::cell::RefCell;

use serde::{Deserialize, Serialize};
use tokio::task_local;

use crate::generated_ports::p_5b257a1951fde::get_session_id;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentContext {
    pub agent_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagent_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_built_in: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invoking_request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invocation_kind: Option<InvocationKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invocation_emitted: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeammateAgentContext {
    pub agent_id: String,
    pub agent_name: String,
    pub team_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_color: Option<String>,
    pub plan_mode_required: bool,
    pub parent_session_id: String,
    pub is_team_lead: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invoking_request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invocation_kind: Option<InvocationKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invocation_emitted: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InvocationKind {
    Spawn,
    Resume,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "agentType", rename_all = "lowercase")]
pub enum AgentContext {
    subagent(SubagentContext),
    teammate(TeammateAgentContext),
}

/// Process-lifetime swarm feature switch (original: isAgentSwarmsEnabled).
pub static AGENT_SWARMS_ENABLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true);

task_local! {
    static AGENT_CONTEXT: RefCell<Option<AgentContext>>;
}

/// Returns the current agent context, if any.
pub fn get_agent_context() -> Option<AgentContext> {
    AGENT_CONTEXT
        .try_with(|slot| slot.borrow().clone())
        .unwrap_or(None)
}

/// Runs `f` with the given agent context visible to the whole task scope
/// (original `runWithAgentContext`).
pub fn run_with_agent_context<R>(context: AgentContext, f: impl FnOnce() -> R) -> R {
    AGENT_CONTEXT.sync_scope(RefCell::new(Some(context)), f)
}

/// Type guard: subagent context.
pub fn is_subagent_context(context: Option<&AgentContext>) -> bool {
    matches!(context, Some(AgentContext::subagent(_)))
}

/// Type guard: teammate context (gated by the swarm feature switch).
pub fn is_teammate_agent_context(context: Option<&AgentContext>) -> bool {
    if !AGENT_SWARMS_ENABLED.load(std::sync::atomic::Ordering::Relaxed) {
        return false;
    }
    matches!(context, Some(AgentContext::teammate(_)))
}

/// Subagent name for analytics logging: built-in names are code constants,
/// custom agents map to the literal "user-defined". None outside a
/// subagent context or when the name is absent.
pub fn get_subagent_log_name() -> Option<String> {
    let context = get_agent_context()?;
    let AgentContext::subagent(sub) = context else {
        return None;
    };
    let name = sub.subagent_name?;
    Some(if sub.is_built_in.unwrap_or(false) {
        name
    } else {
        "user-defined".to_string()
    })
}

/// One-shot consume of the invoking request_id (sparse edge semantics:
/// exactly one emission per spawn/resume boundary). Mutates the stored
/// context's `invocation_emitted` flag, mirroring the original.
pub fn consume_invoking_request_id() -> Option<(String, Option<InvocationKind>)> {
    AGENT_CONTEXT
        .try_with(|slot| {
            let mut current = slot.borrow_mut();
            let context = current.as_mut()?;
            let (request_id, kind) = match context {
                AgentContext::subagent(s) => {
                    let rid = s.invoking_request_id.clone()?;
                    if s.invocation_emitted.unwrap_or(false) {
                        return None;
                    }
                    s.invocation_emitted = Some(true);
                    (rid, s.invocation_kind)
                }
                AgentContext::teammate(t) => {
                    let rid = t.invoking_request_id.clone()?;
                    if t.invocation_emitted.unwrap_or(false) {
                        return None;
                    }
                    t.invocation_emitted = Some(true);
                    (rid, t.invocation_kind)
                }
            };
            Some((request_id, kind))
        })
        .unwrap_or(None)
}

/// Convenience constructor: a subagent context rooted at the current
/// session (parent_session_id defaults to the process session id).
pub fn subagent_context(agent_id: String, subagent_name: Option<String>) -> SubagentContext {
    SubagentContext {
        agent_id,
        parent_session_id: Some(get_session_id()),
        subagent_name,
        is_built_in: None,
        invoking_request_id: None,
        invocation_kind: None,
        invocation_emitted: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_sub(built_in: bool) -> AgentContext {
        AgentContext::subagent(SubagentContext {
            agent_id: "a1234567890abcdef".into(),
            parent_session_id: Some("sess-1".into()),
            subagent_name: Some("Explore".into()),
            is_built_in: Some(built_in),
            invoking_request_id: Some("req-9".into()),
            invocation_kind: Some(InvocationKind::Spawn),
            invocation_emitted: None,
        })
    }

    #[tokio::test]
    async fn context_isolated_per_task() {
        let ctx = sample_sub(true);
        run_with_agent_context(ctx, || {
            assert!(is_subagent_context(get_agent_context().as_ref()));
            assert_eq!(
                get_subagent_log_name().as_deref(),
                Some("Explore"),
                "built-in name passes through"
            );
        });
        // Outside the scope: no context.
        assert!(get_agent_context().is_none());
    }

    #[tokio::test]
    async fn custom_subagent_maps_to_user_defined() {
        let mut sub = match sample_sub(false) {
            AgentContext::subagent(s) => s,
            _ => unreachable!(),
        };
        sub.subagent_name = Some("my-reviewer".into());
        run_with_agent_context(AgentContext::subagent(sub), || {
            assert_eq!(get_subagent_log_name().as_deref(), Some("user-defined"));
        });
    }

    #[tokio::test]
    async fn invoking_request_id_consumed_once() {
        run_with_agent_context(sample_sub(true), || {
            let first = consume_invoking_request_id();
            assert_eq!(first.as_ref().map(|(id, _)| id.as_str()), Some("req-9"));
            assert_eq!(first.unwrap().1, Some(InvocationKind::Spawn));
            // Second consume within the same invocation: None.
            assert!(consume_invoking_request_id().is_none());
        });
    }

    #[tokio::test]
    async fn teammate_guard_gated_by_switch() {
        let teammate = AgentContext::teammate(TeammateAgentContext {
            agent_id: "researcher@team-a".into(),
            agent_name: "researcher".into(),
            team_name: "team-a".into(),
            agent_color: None,
            plan_mode_required: false,
            parent_session_id: "sess-1".into(),
            is_team_lead: false,
            invoking_request_id: None,
            invocation_kind: None,
            invocation_emitted: None,
        });
        run_with_agent_context(teammate, || {
            assert!(is_teammate_agent_context(get_agent_context().as_ref()));
            // Subagent guard is false for teammate contexts.
            assert!(!is_subagent_context(get_agent_context().as_ref()));
        });
    }

    #[test]
    fn serde_tag_dispatch_roundtrip() {
        let sub = sample_sub(true);
        let value = serde_json::to_value(&sub).unwrap();
        assert_eq!(value["agentType"], "subagent");
        let back: AgentContext = serde_json::from_value(value).unwrap();
        assert!(is_subagent_context(Some(&back)));
        let _ = json!(null);
    }
}

/// Registry-view wrapper preserving the original export name.
pub fn isSubagentContext(args: &[serde_json::Value]) -> serde_json::Value {
    let context: Option<AgentContext> = args
        .first()
        .and_then(|v| serde_json::from_value(v.clone()).ok());
    serde_json::Value::Bool(is_subagent_context(context.as_ref()))
}
