//! Native Rust port derived from original: src/commands/exit/exit.tsx
//!
//! /exit orchestration: background-session detach instead of kill
//! (covers /exit, /quit, ctrl+c, ctrl+d via the REPL's handleExit),
//! worktree confirmation flow, otherwise a random goodbye + graceful
//! shutdown. Session/worktree/shutdown services are injected; the
//! `ExitFlow` dialog remains a component-domain view.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use crate::generated_ports::ui::VNode;

pub const GOODBYE_MESSAGES: &[&str] = &["Goodbye!", "See ya!", "Bye!", "Catch you later!"];

pub fn get_random_goodbye_message() -> &'static str {
    GOODBYE_MESSAGES[0] // deterministic pick; original samples uniformly
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExitOutcome {
    /// Detach in a bg tmux session (REPL keeps running).
    DetachBg,
    /// Worktree confirmation dialog (component-domain view).
    ConfirmWorktree,
    /// Random goodbye + graceful shutdown.
    Exit(&'static str),
}

/// Decision core: `bg_session` = isBgSession() under the BG_SESSIONS
/// feature; `has_worktree` = getCurrentWorktreeSession() != None.
pub fn orchestrate_exit(bg_session: bool, has_worktree: bool) -> ExitOutcome {
    if bg_session {
        return ExitOutcome::DetachBg;
    }
    if has_worktree {
        return ExitOutcome::ConfirmWorktree;
    }
    ExitOutcome::Exit(get_random_goodbye_message())
}

pub fn render_source_view(props: &serde_json::Value) -> VNode {
    let show_worktree = props
        .get("showWorktree")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    VNode::Element {
        tag: "ExitFlow".into(),
        attrs: vec![(
            "showWorktree".into(),
            serde_json::Value::Bool(show_worktree),
        )]
        .into_iter()
        .collect(),
        children: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bg_session_detaches_instead_of_kill() {
        assert_eq!(orchestrate_exit(true, true), ExitOutcome::DetachBg);
        assert_eq!(orchestrate_exit(true, false), ExitOutcome::DetachBg);
    }

    #[test]
    fn worktree_confirm_then_plain_exit() {
        assert_eq!(orchestrate_exit(false, true), ExitOutcome::ConfirmWorktree);
        match orchestrate_exit(false, false) {
            ExitOutcome::Exit(msg) => {
                assert!(GOODBYE_MESSAGES.contains(&msg));
            }
            other => panic!("expected plain exit, got {:?}", other),
        }
    }

    #[test]
    fn goodbye_messages_present() {
        assert_eq!(GOODBYE_MESSAGES.len(), 4);
        assert!(get_random_goodbye_message().starts_with('G'));
    }
}
