//! Rust exports derived from original: src/hooks/useTeammateViewAutoExit.ts
//!
//! Auto-exit decision for teammate viewing: leave when the viewed teammate
//! is killed or errored, stay for completed teammates (full transcript
//! review). Selects only the viewed task so unrelated streaming updates
//! never re-trigger.

/// Decision for the current viewing state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeammateViewDecision {
    Stay,
    AutoExit,
}

/// `useEffect` decision core.
pub fn teammate_view_decision(
    viewing: Option<&str>,
    viewed_status: Option<&str>,
    viewed_error: Option<&str>,
    task_exists: bool,
) -> TeammateViewDecision {
    let Some(_id) = viewing else {
        return TeammateViewDecision::Stay;
    };
    if !task_exists {
        // Task evicted (killed) — leave viewing mode.
        return TeammateViewDecision::AutoExit;
    }
    if viewed_error.is_some() {
        return TeammateViewDecision::AutoExit;
    }
    match viewed_status {
        Some("killed") => TeammateViewDecision::AutoExit,
        // Completed teammates stay viewable on purpose.
        _ => TeammateViewDecision::Stay,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_on_killed_or_error_or_evicted() {
        assert_eq!(
            teammate_view_decision(Some("t1"), Some("killed"), None, true),
            TeammateViewDecision::AutoExit
        );
        assert_eq!(
            teammate_view_decision(Some("t1"), Some("running"), Some("boom"), true),
            TeammateViewDecision::AutoExit
        );
        assert_eq!(
            teammate_view_decision(Some("t1"), None, None, false),
            TeammateViewDecision::AutoExit
        );
    }

    #[test]
    fn completed_stays_viewable() {
        assert_eq!(
            teammate_view_decision(Some("t1"), Some("completed"), None, true),
            TeammateViewDecision::Stay
        );
        assert_eq!(
            teammate_view_decision(None, Some("killed"), None, true),
            TeammateViewDecision::Stay
        );
    }
}
