//! Manual Rust port of 2.1.88 utils/teammateContext.ts.
//! Tokio task-local storage gives each concurrent in-process teammate its
//! own identity. It never relies on global environment variables.
use std::future::Future;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::Notify;

#[derive(Clone, Debug, Default)]
pub struct AbortController {
    aborted: Arc<AtomicBool>,
    notification: Arc<Notify>,
}

impl AbortController {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn abort(&self) {
        if !self.aborted.swap(true, Ordering::SeqCst) {
            self.notification.notify_waiters();
        }
    }

    pub fn is_aborted(&self) -> bool {
        self.aborted.load(Ordering::SeqCst)
    }

    pub async fn cancelled(&self) {
        if self.is_aborted() {
            return;
        }
        let notified = self.notification.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        if self.is_aborted() {
            return;
        }
        notified.await;
    }
}

#[derive(Clone, Debug)]
pub struct TeammateContext {
    pub agent_id: String,
    pub agent_name: String,
    pub team_name: String,
    pub color: Option<String>,
    pub plan_mode_required: bool,
    pub parent_session_id: String,
    pub is_in_process: bool,
    pub abort_controller: AbortController,
}

tokio::task_local! {
    static TEAMMATE_CONTEXT: TeammateContext;
}

pub fn create_teammate_context(
    agent_id: String,
    agent_name: String,
    team_name: String,
    color: Option<String>,
    plan_mode_required: bool,
    parent_session_id: String,
    abort_controller: AbortController,
) -> TeammateContext {
    TeammateContext {
        agent_id,
        agent_name,
        team_name,
        color,
        plan_mode_required,
        parent_session_id,
        is_in_process: true,
        abort_controller,
    }
}

pub fn get_teammate_context() -> Option<TeammateContext> {
    TEAMMATE_CONTEXT.try_with(Clone::clone).ok()
}

pub fn is_in_process_teammate() -> bool {
    TEAMMATE_CONTEXT.try_with(|_| ()).is_ok()
}

pub async fn run_with_teammate_context<F: Future>(
    context: TeammateContext,
    future: F,
) -> F::Output {
    TEAMMATE_CONTEXT.scope(context, future).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn concurrent_teammates_have_isolated_contexts() {
        assert!(!is_in_process_teammate());
        assert!(get_teammate_context().is_none());
        let a = create_teammate_context(
            "researcher@team".into(),
            "researcher".into(),
            "team".into(),
            Some("blue".into()),
            false,
            "parent".into(),
            AbortController::new(),
        );
        let b = create_teammate_context(
            "reviewer@team".into(),
            "reviewer".into(),
            "team".into(),
            None,
            true,
            "parent".into(),
            AbortController::new(),
        );
        let futures = [a, b].map(|context| async {
            run_with_teammate_context(context, async {
                assert!(is_in_process_teammate());
                let before = get_teammate_context().unwrap();
                tokio::task::yield_now().await;
                assert_eq!(before.agent_id, get_teammate_context().unwrap().agent_id);
                before.agent_name
            })
            .await
        });
        let results = futures::future::join_all(futures).await;
        assert_eq!(results, ["researcher", "reviewer"]);
        assert!(get_teammate_context().is_none());
    }

    #[tokio::test]
    async fn nested_scope_restores_parent_even_when_child_returns_error() {
        let outer = create_teammate_context(
            "outer@team".into(),
            "outer".into(),
            "team".into(),
            None,
            false,
            "parent".into(),
            AbortController::new(),
        );
        let inner = create_teammate_context(
            "inner@team".into(),
            "inner".into(),
            "team".into(),
            None,
            true,
            "parent".into(),
            AbortController::new(),
        );
        run_with_teammate_context(outer, async {
            assert_eq!(get_teammate_context().unwrap().agent_name, "outer");
            let error: Result<(), &str> = run_with_teammate_context(inner, async {
                assert_eq!(get_teammate_context().unwrap().agent_name, "inner");
                Err("stopped")
            })
            .await;
            assert_eq!(error.unwrap_err(), "stopped");
            assert_eq!(get_teammate_context().unwrap().agent_name, "outer");
        })
        .await;
    }

    #[tokio::test]
    async fn abort_signal_remains_independent_from_parent_scope() {
        let controller = AbortController::new();
        let observer = controller.clone();
        let waiter = tokio::spawn(async move {
            observer.cancelled().await;
            observer.is_aborted()
        });
        controller.abort();
        assert!(waiter.await.unwrap());
        assert!(controller.is_aborted());
    }
}
