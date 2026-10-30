//! Rust port of `src/services/tools/StreamingToolExecutor.ts` (2.1.88).
//!
//! The original executor starts tools while the model response is still
//! streaming: each complete tool_use block enters a queue, concurrency-safe
//! tools may run alongside other in-flight safe tools, unsafe tools wait for
//! exclusive access, and results are buffered so every tool id receives its
//! tool_result in block order. Bash/PowerShell failures cancel in-flight
//! siblings (implicit shell dependency chains); a streaming fallback discards
//! queued work so stale tool ids never leak into a retry.
//!
//! Rust form: the agent layer hands complete `ToolCall`s to [`add_tool`].
//! Each admitted tool runs on a spawned task owning a [`StreamingDeps`]
//! snapshot — hooks, executor, permission inputs are all `Clone + Send +
//! Sync`, mirroring the original's shared ToolUseContext. Calls the executor
//! dispatch table cannot carry (subagents, teams, task graph, MCP, dynamic
//! plugin tools) are recorded as deferred so the agent's serial path runs
//! them with its full special-dispatch branches. Results come back in block
//! order from [`drain`].

use crate::hooks::HookRunner;
use crate::protocol::ToolCall;
use crate::tools::ToolExecutor;
use futures::stream::StreamExt;
use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::Notify;

/// Minimal cancellation token (avoid pulling tokio-util for one primitive):
/// an atomic flag plus a notify handle, checked before and around tool work.
#[derive(Default)]
pub struct CancelToken {
    flag: std::sync::atomic::AtomicBool,
    notify: Notify,
}

impl CancelToken {
    pub fn cancel(&self) {
        self.flag.store(true, std::sync::atomic::Ordering::SeqCst);
        self.notify.notify_waiters();
    }
    pub fn is_cancelled(&self) -> bool {
        self.flag.load(std::sync::atomic::Ordering::SeqCst)
    }
    pub async fn cancelled(&self) {
        while !self.is_cancelled() {
            let notified = self.notify.notified();
            if self.is_cancelled() {
                return;
            }
            notified.await;
        }
    }
}

/// Tool calls the executor dispatch table can carry end to end. Everything
/// else is deferred to the agent's serial path (special dispatch branches
/// require exclusive agent access), matching the original behavior where
/// those tools simply execute through runToolUse with full context.
fn executor_handled(name: &str) -> bool {
    matches!(
        crate::tool_names::canonical_name(name),
        "run_command"
            | "read_file"
            | "edit_file"
            | "write_file"
            | "notebook_edit"
            | "glob_files"
            | "grep_files"
            | "search_text"
            | "web_fetch"
            | "web_search"
            | "list_files"
            | "list_skills"
            | "read_skill"
            | "task_output"
            | "file_history"
            | "file_history_diff"
            | "file_revert"
            | "file_rewind"
            | "git_diff"
            | "git_log"
            | "git_status"
            | "background_list"
    ) || name == "PowerShell"
}

/// Shell tools whose failure cancels in-flight siblings: shell commands
/// often carry implicit dependency chains (mkdir before writes), while
/// independent reads do not.
fn is_shell(name: &str) -> bool {
    crate::tool_names::canonical_name(name) == "run_command" || name == "PowerShell"
}

/// `isConcurrencySafe` for the streaming admission check — same table as
/// the batch orchestration path.
fn concurrency_safe(name: &str) -> bool {
    crate::tool_orchestration::is_concurrency_safe(name)
}

/// `getToolDescription` — command, then file path, then pattern, truncated
/// to 40 characters with an ellipsis, for cancel/discard messages.
pub fn tool_description(call: &ToolCall) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(&call.function.arguments).ok()?;
    let raw = ["command", "file_path", "pattern"]
        .iter()
        .find_map(|key| value.get(*key).and_then(serde_json::Value::as_str))?;
    let truncated: String = raw.chars().take(40).collect();
    Some(if raw.chars().count() > 40 {
        format!("{truncated}…")
    } else {
        truncated
    })
}

/// Immutable execution context shared by spawned tool tasks, snapshotted at
/// executor construction (the values are invariant within one model turn).
pub struct StreamingDeps {
    pub hooks: HookRunner,
    pub executor: ToolExecutor,
    pub tool_hook_base: serde_json::Value,
    pub permission_mode_label: String,
    pub prompt_role: Option<String>,
    pub child_depth: u8,
    pub session_id: String,
    pub structured_output: bool,
    pub sdk_allowed_tools: Option<Vec<String>>,
    pub sdk_disallowed_tools: HashSet<String>,
    /// Cancelled when a shell sibling fails; in-flight tasks then return the
    /// original's "Cancelled: parallel tool call …" synthetic error.
    pub bash_cancel: CancelToken,
    /// Description of the shell tool whose error triggered the cascade.
    pub errored_description: Arc<Mutex<Option<String>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolStatus {
    Queued,
    Executing,
    Completed,
    Yielded,
}

struct Tracked {
    call: ToolCall,
    status: ToolStatus,
    /// Set for calls the executor cannot carry; the agent's serial path
    /// must run them after the drain.
    deferred: bool,
    outcome: Option<ToolOutcome>,
    join: Option<tokio::task::JoinHandle<ToolOutcomeInternal>>,
}

#[derive(Debug, Clone)]
pub struct ToolOutcome {
    pub call: ToolCall,
    pub result: String,
    pub errored: bool,
    pub started: Instant,
}

struct ToolOutcomeInternal {
    call: ToolCall,
    result: String,
    errored: bool,
    started: Instant,
}

impl From<ToolOutcomeInternal> for ToolOutcome {
    fn from(v: ToolOutcomeInternal) -> Self {
        ToolOutcome {
            call: v.call,
            result: v.result,
            errored: v.errored,
            started: v.started,
        }
    }
}

/// `StreamingToolExecutor`.
pub struct StreamingExecutor {
    deps: Arc<StreamingDeps>,
    tools: Vec<Tracked>,
    /// Set when a shell sibling error starts the cancellation cascade.
    has_errored: bool,
    /// `discard()` — a streaming fallback abandons every queued/in-flight
    /// call with the original's synthetic error so old ids never leak.
    discarded: bool,
}

impl StreamingExecutor {
    pub fn new(deps: StreamingDeps) -> Self {
        Self {
            deps: Arc::new(deps),
            tools: Vec::new(),
            has_errored: false,
            discarded: false,
        }
    }

    /// `canExecuteTool` — a tool may start when nothing is executing, or
    /// when it is concurrency-safe and every in-flight tool is safe too.
    fn can_execute(&self, safe: bool) -> bool {
        let executing: Vec<&Tracked> = self
            .tools
            .iter()
            .filter(|t| t.status == ToolStatus::Executing)
            .collect();
        executing.is_empty()
            || (safe
                && executing
                    .iter()
                    .all(|t| concurrency_safe(&t.call.function.name)))
    }

    /// `addTool` — unknown tools to the executor become deferred (the agent
    /// serial path carries their full semantics); others enter the queue and
    /// start immediately when admission allows.
    pub fn add_tool(&mut self, call: ToolCall) {
        let safe = concurrency_safe(&call.function.name);
        let handled = executor_handled(&call.function.name);
        let index = self.tools.len();
        self.tools.push(Tracked {
            call,
            status: ToolStatus::Queued,
            deferred: !handled,
            outcome: None,
            join: None,
        });
        if self.discarded {
            self.finish_with_error(index, "ERROR: Streaming fallback - tool execution discarded");
        } else if handled && self.can_execute(safe) {
            self.start(index);
        }
    }

    /// Calls the agent must still run through its serial special-dispatch
    /// path after the drain, in block order.
    pub fn deferred_tools(&self) -> Vec<ToolCall> {
        self.tools
            .iter()
            .filter(|t| t.deferred)
            .map(|t| t.call.clone())
            .collect()
    }

    /// `processQueue` — start every queued tool whose admission check now
    /// passes. A non-safe tool behind an executing tool keeps the order
    /// (queued tools are processed in block order and stop at the first
    /// blocked non-safe call, mirroring the original break).
    fn process_queue(&mut self) {
        for i in 0..self.tools.len() {
            let tracked = &self.tools[i];
            if tracked.status != ToolStatus::Queued || tracked.deferred {
                continue;
            }
            if self.has_errored || self.discarded {
                break;
            }
            let safe = concurrency_safe(&tracked.call.function.name);
            if self.can_execute(safe) {
                self.start(i);
            } else {
                // Preserve ordering across pending exclusive tool uses.
                break;
            }
        }
    }

    // Keep the same tracked entry in its original tool-block position.
    fn start(&mut self, index: usize) {
        let tracked = &mut self.tools[index];
        debug_assert_eq!(tracked.status, ToolStatus::Queued);
        tracked.status = ToolStatus::Executing;
        let deps = Arc::clone(&self.deps);
        let call = tracked.call.clone();
        tracked.join = Some(tokio::spawn(async move { run_streamed_tool(&deps, call).await }));
    }

    fn finish_with_error(&mut self, index: usize, error: &str) {
        let tracked = &mut self.tools[index];
        if let Some(join) = tracked.join.take() {
            join.abort();
        }
        tracked.status = ToolStatus::Completed;
        tracked.outcome = Some(ToolOutcome {
            call: tracked.call.clone(),
            result: error.to_owned(),
            errored: true,
            started: Instant::now(),
        });
    }

    fn cancel_siblings_after_shell_error(&mut self, failed_index: usize) {
        self.has_errored = true;
        let description = tool_description(&self.tools[failed_index].call);
        *self.deps.errored_description.lock().unwrap() = description.clone();
        self.deps.bash_cancel.cancel();
        let reason = match description {
            Some(description) => format!("ERROR: Cancelled: parallel tool call {description} errored"),
            None => "ERROR: Cancelled: parallel tool call errored".to_owned(),
        };
        for index in 0..self.tools.len() {
            if index != failed_index
                && matches!(self.tools[index].status, ToolStatus::Queued | ToolStatus::Executing)
            {
                self.finish_with_error(index, &reason);
            }
        }
    }

    /// React to whichever task finishes first, especially a shell error.
    /// Completed results are still emitted in their source tool-block order.
    pub async fn drain(&mut self) -> Vec<ToolOutcome> {
        loop {
            self.process_queue();
            if self.tools.iter().all(|t| {
                matches!(t.status, ToolStatus::Completed | ToolStatus::Yielded) || t.deferred
            }) {
                break;
            }
            type Completion<'a> = Pin<Box<dyn Future<Output =
                (usize, Result<ToolOutcomeInternal, tokio::task::JoinError>)> + Send + 'a>>;
            let finished = {
                let waits: Vec<Completion<'_>> = self.tools.iter_mut().enumerate()
                    .filter_map(|(index, tracked)| {
                        if tracked.status != ToolStatus::Executing {
                            return None;
                        }
                        tracked.join.as_mut().map(|join| {
                            Box::pin(async move { (index, join.await) }) as Completion<'_>
                        })
                    }).collect();
                if waits.is_empty() {
                    None
                } else {
                    Some(futures::future::select_all(waits).await.0)
                }
            };
            if let Some((index, completed)) = finished {
                self.tools[index].join.take();
                let outcome = match completed {
                    Ok(result) => result.into(),
                    Err(error) => ToolOutcome {
                        call: self.tools[index].call.clone(),
                        result: format!("ERROR: Tool execution task failed: {error}"),
                        errored: true,
                        started: Instant::now(),
                    },
                };
                let shell_failed = outcome.errored && is_shell(&outcome.call.function.name);
                self.tools[index].status = ToolStatus::Completed;
                self.tools[index].outcome = Some(outcome);
                if shell_failed {
                    self.cancel_siblings_after_shell_error(index);
                }
            } else {
                // Missing join handles must cause visible failures, not a spin loop.
                for index in 0..self.tools.len() {
                    if matches!(self.tools[index].status, ToolStatus::Queued | ToolStatus::Executing)
                        && !self.tools[index].deferred
                    {
                        self.finish_with_error(index, "ERROR: Tool execution lost its task handle");
                    }
                }
            }
        }
        let mut outcomes = Vec::new();
        for tracked in &mut self.tools {
            if tracked.status == ToolStatus::Completed {
                tracked.status = ToolStatus::Yielded;
                if let Some(outcome) = tracked.outcome.take() {
                    outcomes.push(outcome);
                }
            }
        }
        outcomes
    }

    /// Cancel queued and in-flight tools when provider streaming fails.
    /// Dropping a JoinHandle alone would detach its dangerous side effects.
    pub fn discard(&mut self) {
        self.discarded = true;
        self.deps.bash_cancel.cancel();
        for index in 0..self.tools.len() {
            if matches!(self.tools[index].status, ToolStatus::Queued | ToolStatus::Executing) {
                self.finish_with_error(index, "ERROR: Streaming fallback - tool execution discarded");
            }
        }
    }

}

/// One tool execution: validation + PreToolUse hook (with the original's
/// forced-approval semantics) + executor call + PostToolUse hook. Runs on
/// the shared deps snapshot; stdout emit mirrors `Agent::emit`.
async fn run_streamed_tool(deps: &Arc<StreamingDeps>, mut call: ToolCall) -> ToolOutcomeInternal {
    let started = Instant::now();
    if deps.bash_cancel.is_cancelled() {
        let desc = deps.errored_description.lock().unwrap().clone();
        let message = match desc.as_deref() {
            Some(desc) => format!("ERROR: Cancelled: parallel tool call {desc} errored"),
            None => "ERROR: Cancelled: parallel tool call errored".to_owned(),
        };
        return ToolOutcomeInternal {
            call,
            result: message,
            errored: true,
            started,
        };
    }
    let hook_input = {
        let mut input = deps.tool_hook_base.clone();
        input["permission_mode"] = serde_json::json!(deps.permission_mode_label);
        if let Some(role) = deps.prompt_role.as_deref() {
            input["agent_type"] = serde_json::json!(role);
        }
        if deps.child_depth > 0 {
            input["agent_id"] = serde_json::json!(deps.session_id);
        }
        input
    };
    let restricted = (deps.child_depth > 0
        && deps.prompt_role.as_deref().is_some_and(|role| {
            !crate::prompt_runtime::tool_allowed_for_role(role, &call.function.name)
        }))
        || deps.sdk_allowed_tools.as_ref().is_some_and(|allowed| {
            !allowed.iter().any(|entry| {
                entry == "*"
                    || entry == &call.function.name
                    || entry == crate::tool_names::canonical_name(&call.function.name)
            })
        })
        || deps.sdk_disallowed_tools.contains(&call.function.name)
        || deps
            .sdk_disallowed_tools
            .contains(crate::tool_names::canonical_name(&call.function.name));
    let result = if restricted {
        format!(
            "ERROR: Tool {} is unavailable to this agent's tool policy",
            call.function.name
        )
    } else {
        // PreToolUse stage: a failed or blocking hook never executes the
        // tool, mirroring the serial path's synthesized pre-execution errors.
        let pre_failure: Option<String> = match deps
            .hooks
            .before_tool_with_context(&call, hook_input.clone())
            .await
        {
            Err(error) => Some(format!("ERROR: PreToolUse hook failed: {error:#}")),
            Ok(outcome) => {
                if let Some(updated) = outcome.updated_input.as_ref() {
                    call.function.arguments = serde_json::to_string(updated)
                        .unwrap_or_else(|_| call.function.arguments.clone());
                }
                outcome
                    .blocking_reason
                    .map(|reason| format!("ERROR: PreToolUse hook blocked: {reason}"))
            }
        };
        if let Some(reason) = pre_failure {
            reason
        } else {
            let raw = deps.executor.execute(&call).await;
            let (post, mut result): (crate::hooks::HookOutcome, String) = match deps
                .hooks
                .after_tool_with_context(&call, &raw, hook_input)
                .await
            {
                Ok(outcome) => (outcome, raw),
                Err(error) => (
                    Default::default(),
                    format!(
                        "ERROR: PostToolUse hook failed after tool execution: {error:#}\nTool execution result: {raw}"
                    ),
                ),
            };
            if let Some(reason) = post.blocking_reason {
                result.push_str(&format!(
                    "\nPostToolUse hook blocked continuation: {reason}"
                ));
            }
            result
        }
    };
    let errored = result.starts_with("ERROR:");
    ToolOutcomeInternal {
        call,
        result,
        errored,
        started,
    }
}

/// Consume the live tool-block channel until the response completes.
pub async fn pump_blocks(
    mut blocks: UnboundedReceiver<ToolCall>,
    executor: Arc<Mutex<StreamingExecutor>>,
) {
    while let Some(call) = blocks.recv().await {
        let _ = executor.lock().unwrap().add_tool(call);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::FunctionCall;
    use crate::tools::Approval;
    use serde_json::json;
    use std::path::PathBuf;
    use tokio::time::{sleep, timeout, Duration};

    fn deps() -> StreamingDeps {
        StreamingDeps {
            hooks: HookRunner::new(&[]).expect("empty hook configuration"),
            executor: ToolExecutor::new(PathBuf::from("."), Approval::Full, false),
            tool_hook_base: json!({}),
            permission_mode_label: "default".to_owned(),
            prompt_role: None,
            child_depth: 0,
            session_id: "audit".to_owned(),
            structured_output: false,
            sdk_allowed_tools: None,
            sdk_disallowed_tools: HashSet::new(),
            bash_cancel: CancelToken::default(),
            errored_description: Arc::new(Mutex::new(None)),
        }
    }
    fn call(name: &str, id: &str) -> ToolCall {
        ToolCall {
            id: id.to_owned(),
            kind: "function".to_owned(),
            function: FunctionCall {
                name: name.to_owned(),
                arguments: "{}".to_owned(),
            },
        }
    }
    fn simulated(call: ToolCall, delay_ms: u64, errored: bool) -> tokio::task::JoinHandle<ToolOutcomeInternal> {
        tokio::spawn(async move {
            sleep(Duration::from_millis(delay_ms)).await;
            ToolOutcomeInternal {
                call,
                result: if errored { "ERROR: simulated shell failure" } else { "ok" }.to_owned(),
                errored,
                started: Instant::now(),
            }
        })
    }
    fn executing(call: ToolCall, task: tokio::task::JoinHandle<ToolOutcomeInternal>) -> Tracked {
        Tracked {
            call,
            status: ToolStatus::Executing,
            deferred: false,
            outcome: None,
            join: Some(task),
        }
    }

    #[tokio::test]
    async fn queued_unsafe_tool_runs_once_and_second_drain_is_empty() {
        let mut executor = StreamingExecutor::new(deps());
        let first = call("run_command", "a");
        let task = simulated(first.clone(), 30, false);
        executor.tools.push(executing(first, task));
        executor.add_tool(call("run_command", "b"));
        assert_eq!(executor.tools.len(), 2);
        assert_eq!(executor.tools[1].status, ToolStatus::Queued);
        let result = timeout(Duration::from_secs(3), executor.drain())
            .await.expect("queue drain must terminate");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].call.id, "a");
        assert_eq!(result[1].call.id, "b");
        assert_eq!(executor.tools.len(), 2, "must not append a second copy of b");
        assert!(executor.drain().await.is_empty(), "results are once-only");
    }

    #[tokio::test]
    async fn shell_error_closes_queued_and_deferred_tools_without_hanging() {
        let mut executor = StreamingExecutor::new(deps());
        let first = call("run_command", "failed");
        executor.tools.push(executing(first.clone(), simulated(first, 20, true)));
        executor.add_tool(call("run_command", "blocked"));
        executor.add_tool(call("nonexistent_agent_tool", "deferred"));
        let results = timeout(Duration::from_secs(2), executor.drain())
            .await.expect("shell failure must not strand queued tools");
        assert_eq!(results.iter().map(|r| r.call.id.as_str()).collect::<Vec<_>>(),
            vec!["failed", "blocked", "deferred"]);
        assert!(results[1].result.contains("Cancelled"));
        assert!(results[2].result.contains("Cancelled"));
        assert!(executor.deps.bash_cancel.is_cancelled());
    }

    #[tokio::test]
    async fn task_panic_produces_visible_error_and_terminates() {
        let mut executor = StreamingExecutor::new(deps());
        let first = call("read_file", "panic");
        let task = tokio::spawn(async move { panic!("intentional audit panic") });
        executor.tools.push(executing(first, task));
        let results = timeout(Duration::from_secs(2), executor.drain())
            .await.expect("panicked task must not loop forever");
        assert_eq!(results.len(), 1);
        assert!(results[0].errored);
        assert!(results[0].result.contains("task failed"));
    }

    #[tokio::test]
    async fn discarding_stream_aborts_in_flight_side_effects() {
        let folder = tempfile::tempdir().unwrap();
        let target = folder.path().join("must_not_exist");
        let mut executor = StreamingExecutor::new(deps());
        let async_call = call("write_file", "pending_write");
        let task = tokio::spawn(async move {
            sleep(Duration::from_millis(200)).await;
            std::fs::write(&target, "unexpected").unwrap();
            ToolOutcomeInternal { call: async_call, result: "written".to_owned(), errored: false,
                started: Instant::now() }
        });
        let call = call("write_file", "pending_write");
        executor.tools.push(executing(call, task));
        executor.discard();
        sleep(Duration::from_millis(250)).await;
        assert!(!folder.path().join("must_not_exist").exists());
        assert_eq!(executor.drain().await.len(), 1);
        assert!(executor.drain().await.is_empty());
    }
}
