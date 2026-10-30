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
        let mut tracked = Tracked {
            call,
            status: ToolStatus::Queued,
            deferred: !handled,
            outcome: None,
            join: None,
        };
        if !tracked.deferred && !self.discarded && self.can_execute(safe) {
            self.start(tracked)
        } else {
            self.tools.push(tracked);
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
                let mut taken = Tracked {
                    call: tracked.call.clone(),
                    status: ToolStatus::Queued,
                    deferred: false,
                    outcome: None,
                    join: None,
                };
                std::mem::swap(&mut taken, &mut self.tools[i]);
                self.start(taken);
            } else if !safe {
                break;
            }
        }
    }

    fn start(&mut self, mut tracked: Tracked) {
        tracked.status = ToolStatus::Executing;
        let deps = Arc::clone(&self.deps);
        let call = tracked.call.clone();
        let join = tokio::spawn(async move { run_streamed_tool(&deps, call).await });
        tracked.join = Some(join);
        self.tools.push(tracked);
    }

    /// `getRemainingResults` — start queued work, wait for completions, and
    /// return every outcome in block order (deferred calls surface with
    /// their marker so the agent serial path runs them afterward).
    pub async fn drain(&mut self) -> Vec<ToolOutcome> {
        loop {
            self.process_queue();
            // Collect completed executions (spawned tasks) in block order.
            for i in 0..self.tools.len() {
                if self.tools[i].status == ToolStatus::Executing {
                    if let Some(join) = self.tools[i].join.take() {
                        if let Ok(outcome) = join.await {
                            if outcome.errored && is_shell(&outcome.call.function.name) {
                                self.has_errored = true;
                                let mut slot = self.deps.errored_description.lock().unwrap();
                                if slot.is_none() {
                                    *slot = tool_description(&outcome.call);
                                }
                            }
                            self.tools[i].status = ToolStatus::Completed;
                            self.tools[i].outcome = Some(outcome.into());
                        }
                    }
                }
            }
            if self.tools.iter().all(|t| {
                t.status == ToolStatus::Completed || t.status == ToolStatus::Yielded || t.deferred
            }) {
                break;
            }
            // Everything still executing: yield control so spawns advance.
            tokio::task::yield_now().await;
        }
        // Emit results in block order, marking them yielded.
        let mut outcomes = Vec::new();
        for tracked in &mut self.tools {
            if tracked.status == ToolStatus::Completed {
                tracked.status = ToolStatus::Yielded;
                if let Some(outcome) = tracked.outcome.take() {
                    outcomes.push(outcome);
                }
            } else if tracked.deferred {
                // Deferred calls have no outcome; the agent runs them later.
            }
        }
        outcomes
    }

    /// `discard` — a streaming fallback abandons queued and in-flight work.
    pub fn discard(&mut self) {
        self.discarded = true;
        for tracked in &mut self.tools {
            match tracked.status {
                ToolStatus::Queued => {
                    tracked.status = ToolStatus::Completed;
                    tracked.outcome = Some(
                        ToolOutcomeInternal {
                            call: tracked.call.clone(),
                            result: "ERROR: Streaming fallback - tool execution discarded"
                                .to_owned(),
                            errored: true,
                            started: Instant::now(),
                        }
                        .into(),
                    );
                }
                ToolStatus::Executing => {
                    tracked.status = ToolStatus::Completed;
                    tracked.outcome = Some(
                        ToolOutcomeInternal {
                            call: tracked.call.clone(),
                            result: "ERROR: Streaming fallback - tool execution discarded"
                                .to_owned(),
                            errored: true,
                            started: Instant::now(),
                        }
                        .into(),
                    );
                    tracked.join = None;
                }
                _ => {}
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
