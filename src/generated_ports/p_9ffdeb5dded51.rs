//! Rust exports derived from original: src/state/selectors.ts
//!
//! Pure selectors over AppState — data extraction only, no side effects.
//! The teammate-task narrowing (viewingAgentTaskId → tasks lookup →
//! in-process type guard) is fully portable; the rest of the AppState
//! shape rides the app-state host.

use std::collections::BTreeMap;

/// Minimal task shape the selector narrows on (in-process teammate task).
#[derive(Debug, Clone, PartialEq)]
pub struct InProcessTeammateTaskState {
    pub agent_type: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskKind {
    LocalAgent,
    InProcessTeammate(InProcessTeammateTaskState),
}

/// Selector input slice.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectableAppState<'a> {
    pub viewing_agent_task_id: Option<&'a str>,
    pub tasks: &'a BTreeMap<String, TaskKind>,
}

/// `getViewedTeammateTask` — undefined when not viewing, the id is missing,
/// or the task is not an in-process teammate task.
pub fn get_viewed_teammate_task<'a>(
    app_state: &'a SelectableAppState<'a>,
) -> Option<&'a InProcessTeammateTaskState> {
    let id = app_state.viewing_agent_task_id?;
    match app_state.tasks.get(id)? {
        TaskKind::InProcessTeammate(state) => Some(state),
        TaskKind::LocalAgent => None,
    }
}

/// Convenience constructor mirroring AppState field access.
pub fn state<'a>(
    viewing_agent_task_id: Option<&'a str>,
    tasks: &'a BTreeMap<String, TaskKind>,
) -> SelectableAppState<'a> {
    SelectableAppState {
        viewing_agent_task_id,
        tasks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrowing_semantics() {
        let mut tasks: BTreeMap<String, TaskKind> = BTreeMap::new();
        tasks.insert(
            "t1".into(),
            TaskKind::InProcessTeammate(InProcessTeammateTaskState {
                agent_type: "reviewer".into(),
            }),
        );
        tasks.insert("t2".into(), TaskKind::LocalAgent);
        let s1 = state(Some("t1"), &tasks);
        assert!(get_viewed_teammate_task(&s1).is_some());
        let s2 = state(Some("t2"), &tasks);
        assert!(get_viewed_teammate_task(&s2).is_none());
        let s3 = state(None, &tasks);
        assert!(get_viewed_teammate_task(&s3).is_none());
        let s4 = state(Some("missing"), &tasks);
        assert!(get_viewed_teammate_task(&s4).is_none());
    }
}
