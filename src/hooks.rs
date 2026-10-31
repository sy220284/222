use crate::protocol::ToolCall;
use anyhow::{bail, Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
    time::timeout,
};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum HookEvent {
    SessionStart,
    SessionEnd,
    CwdChanged,
    UserPromptSubmit,
    PreToolUse,
    PostToolUse,
    PostToolUseFailure,
    Stop,
    PreCompact,
    PostCompact,
    TaskCreated,
    TaskCompleted,
    SubagentStart,
    SubagentStop,
    TeammateIdle,
    PermissionRequest,
    PermissionDenied,
    Notification,
    ConfigChange,
    InstructionsLoaded,
    FileChanged,
    Elicitation,
    ElicitationResult,
    StopFailure,
    Setup,
    WorktreeCreate,
    WorktreeRemove,
}

impl HookEvent {
    pub fn name(&self) -> &'static str {
        match self {
            Self::SessionStart => "SessionStart",
            Self::SessionEnd => "SessionEnd",
            Self::CwdChanged => "CwdChanged",
            Self::UserPromptSubmit => "UserPromptSubmit",
            Self::PreToolUse => "PreToolUse",
            Self::PostToolUse => "PostToolUse",
            Self::PostToolUseFailure => "PostToolUseFailure",
            Self::Stop => "Stop",
            Self::PreCompact => "PreCompact",
            Self::PostCompact => "PostCompact",
            Self::TaskCreated => "TaskCreated",
            Self::TaskCompleted => "TaskCompleted",
            Self::SubagentStart => "SubagentStart",
            Self::SubagentStop => "SubagentStop",
            Self::TeammateIdle => "TeammateIdle",
            Self::PermissionRequest => "PermissionRequest",
            Self::PermissionDenied => "PermissionDenied",
            Self::Notification => "Notification",
            Self::ConfigChange => "ConfigChange",
            Self::InstructionsLoaded => "InstructionsLoaded",
            Self::FileChanged => "FileChanged",
            Self::Elicitation => "Elicitation",
            Self::ElicitationResult => "ElicitationResult",
            Self::StopFailure => "StopFailure",
            Self::Setup => "Setup",
            Self::WorktreeCreate => "WorktreeCreate",
            Self::WorktreeRemove => "WorktreeRemove",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HookRule {
    /// Original hook configuration: async runs the process in background.
    #[serde(default, rename = "async")]
    pub async_background: bool,
    /// asyncRewake implies async; completion with exit code 2 asks the model
    /// to continue work on its next wake-up.
    #[serde(default, rename = "asyncRewake")]
    pub async_rewake: bool,
    pub event: HookEvent,
    /// Regular expression against tool name for tool events.
    #[serde(default)]
    pub matcher: Option<String>,
    /// Original hooks.ts `if` permission-style condition. Only tool events
    /// with a validated tool input may satisfy a content-bearing condition.
    #[serde(default, rename = "if", skip_serializing_if = "Option::is_none")]
    pub if_condition: Option<String>,
    /// Internal origin, attached only by the verified plugin loader. It is
    /// never accepted from user hook JSON or exposed in serialized settings.
    #[serde(skip)]
    pub plugin_root: Option<PathBuf>,
    /// User-configured local command. Never downloaded or installed automatically.
    pub command: String,
    /// The source selects a shell per hook; legacy hooks default to Bash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell: Option<crate::shell_runtime::ShellKind>,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
}

fn default_timeout() -> u64 {
    // 2.1.88 hooks.ts uses a 10-minute default for ordinary command hooks.
    600
}

/// SessionEnd is a shutdown path: the original gives all teardown hooks
/// a separate 1.5-second bound, configurable for slow user cleanup scripts.
fn session_end_timeout() -> Duration {
    let millis = std::env::var("ATLAS_SESSIONEND_HOOKS_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(1_500);
    Duration::from_millis(millis)
}

#[derive(Default, Debug, Clone)]
pub struct HookOutcome {
    pub blocking_reason: Option<String>,
    /// Every independent blocking hook, retained in configured order for
    /// TaskCreated/TaskCompleted feedback. Other callers may use the first.
    pub blocking_reasons: Vec<String>,
    pub additional_context: Vec<String>,
    /// SessionStart may enqueue the first user turn for headless execution.
    pub initial_user_message: Option<String>,
    /// FileChanged/CwdChanged hooks may replace dynamically watched paths.
    pub watch_paths: Vec<String>,
    pub updated_input: Option<Value>,
    /// PreToolUse hook decision. An explicit `ask` requires user approval
    /// even when the CLI normally grants full tool access.
    pub permission_decision: Option<String>,
    /// Replacement for an MCP tool's model-visible output from PostToolUse.
    pub updated_mcp_tool_output: Option<Value>,
    pub prevent_continuation: bool,
    /// Original hook JSON stopReason, retained for SDK and CLI stop feedback.
    pub stop_reason: Option<String>,
    /// Non-blocking command-hook failures remain visible without denying the tool.
    pub non_blocking_errors: Vec<String>,
}

impl HookOutcome {
    /// All tool schedulers must honor an explicit hook `ask`, even when
    /// ordinary tools are allowed without prompting. This does not replace
    /// the executor's own permission policy.
    pub fn require_tool_approval(&self, interactive: bool, tool_name: &str) -> Result<()> {
        if self.permission_decision.as_deref() != Some("ask") {
            return Ok(());
        }
        if !interactive {
            bail!("PreToolUse hook requires interactive approval");
        }
        use std::io::Write;
        eprintln!("PreToolUse hook requests approval for {tool_name}");
        eprint!("Allow once? [y/N] ");
        std::io::stderr()
            .flush()
            .context("PreToolUse approval failed")?;
        let mut response = String::new();
        std::io::stdin()
            .read_line(&mut response)
            .context("PreToolUse approval failed")?;
        if !matches!(response.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
            bail!("User rejected PreToolUse hook approval");
        }
        Ok(())
    }
}

/// Source hook matchers use exact tool names for simple strings, not
/// substring regexes. A missing event query bypasses matcher filtering.
#[derive(Clone)]
enum HookMatcher {
    Any,
    Exact(Vec<String>),
    Pattern(Regex),
    Invalid,
}

fn canonical_hook_tool(name: &str) -> &str {
    // The source normalizes legacy Task/KillShell/Output names before matching.
    // The Rust executor also uses internal names for some of these tools.
    match crate::permission_rules::normalize_legacy_tool(name) {
        "Read" => "read_file",
        "Write" => "write_file",
        "Edit" => "edit_file",
        "spawn_agent" | "spawn_team" | "Agent" => "Agent",
        other => crate::tool_names::canonical_name(other),
    }
}

fn source_hook_tool(name: &str) -> Option<&'static str> {
    match canonical_hook_tool(name) {
        "read_file" => Some("Read"),
        "write_file" => Some("Write"),
        "edit_file" => Some("Edit"),
        "run_command" => Some("Bash"),
        "glob_files" => Some("Glob"),
        "grep_files" => Some("Grep"),
        "web_fetch" => Some("WebFetch"),
        "web_search" => Some("WebSearch"),
        "task_output" => Some("TaskOutput"),
        "task_stop" => Some("TaskStop"),
        "notebook_edit" => Some("NotebookEdit"),
        "Agent" => Some("Agent"),
        _ => None,
    }
}

impl HookMatcher {
    fn compile(value: Option<&str>) -> Self {
        let Some(value) = value.filter(|v| !v.is_empty() && *v != "*") else {
            return Self::Any;
        };
        if value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'|')
        {
            return Self::Exact(value.split('|').map(str::to_owned).collect());
        }
        match Regex::new(value) {
            Ok(pattern) => Self::Pattern(pattern),
            Err(_) => Self::Invalid,
        }
    }

    fn matches(&self, query: Option<&str>) -> bool {
        // hooks.ts::getMatchingHooks does not filter matchers at all when
        // the event has no query (Stop, TaskCreated, TaskCompleted, etc.).
        let Some(query) = query.filter(|query| !query.is_empty()) else {
            return true;
        };
        match self {
            Self::Any => true,
            Self::Invalid => false,
            Self::Exact(names) => names.iter().any(|name| {
                name.as_str() == query || canonical_hook_tool(name) == canonical_hook_tool(query)
            }),
            Self::Pattern(pattern) => {
                let source_name = source_hook_tool(query).unwrap_or(query);
                pattern.is_match(query)
                    || pattern.is_match(source_name)
                    // hooks.ts::matchesPattern also tests legacy aliases for
                    // canonical tools. Keep old user-configured hook regexes
                    // effective after the Rust tool-name migration.
                    || crate::permission_rules::legacy_names(source_name)
                        .iter()
                        .any(|name| pattern.is_match(name))
            }
        }
    }
}

/// Evaluate conditional hooks against the same live tool arguments that are
/// passed to the hook. Unknown or unsupported input patterns do not match;
/// they must never turn a conditional command into an unconditional hook.
fn hook_if_matches(condition: &str, event: &str, tool_name: Option<&str>, input: &Value) -> bool {
    if !matches!(
        event,
        "PreToolUse" | "PostToolUse" | "PostToolUseFailure" | "PermissionRequest"
    ) {
        return false;
    }
    let Some(name) = tool_name.or_else(|| input.get("tool_name").and_then(Value::as_str)) else {
        return false;
    };
    let parsed = crate::permission_rules::parse_rule(condition);
    if crate::permission_rules::canonical_tool(&parsed.tool_name)
        != crate::permission_rules::canonical_tool(name)
    {
        return false;
    }
    if parsed.rule_content.is_none() {
        return true;
    }
    let Some(arguments) = input.get("tool_input").filter(|v| v.is_object()) else {
        return false;
    };
    // The hook receives original Read/Edit/Write parameter names such as
    // file_path, while the shared Rust permission matcher expects path.
    // Normalize a copy only for matching; preserve original hook stdin.
    let mut matching_arguments = arguments.clone();
    if matches!(name, "Read" | "Edit" | "Write")
        && crate::semantic_inputs::normalize_source_file_tool(name, &mut matching_arguments)
            .is_err()
    {
        return false;
    }
    let condition_rule = crate::permission_rules::Rule {
        behavior: crate::permission_rules::Behavior::Allow,
        value: parsed,
    };
    if name == "PowerShell" {
        return matching_arguments
            .get("command")
            .and_then(Value::as_str)
            .is_some_and(|command| {
                matches!(
                    crate::powershell_permission_matching::policy_for(&[condition_rule], command),
                    Ok(Some(crate::permission_rules::Behavior::Allow))
                )
            });
    }
    matches!(
        crate::permission_rules::policy_for(&[condition_rule], name, &matching_arguments),
        Ok(Some(crate::permission_rules::Behavior::Allow))
    )
}

#[derive(Default, Clone)]
pub struct HookRunner {
    rules: Vec<(HookRule, HookMatcher)>,
    workspace: Option<PathBuf>,
    /// Captured before a delegated worktree can be removed. SessionEnd hooks
    /// still need a valid cwd after their temporary workspace disappears.
    original_cwd: Option<PathBuf>,
    /// Stable project root inherited by delegated worktree sessions.
    project_root: Option<PathBuf>,
}

impl HookRunner {
    pub fn new(rules: &[HookRule]) -> Result<Self> {
        let mut compiled = Vec::with_capacity(rules.len());
        for rule in rules {
            if rule.command.trim().is_empty() {
                bail!("Hook command cannot be empty");
            }
            // hooks.ts::execCommandHook uses a zero/absent timeout as the
            // default and accepts user timeouts longer than ten minutes.
            // Rejecting them here prevented the entire session from starting.
            let mut rule = rule.clone();
            if rule.timeout_seconds == 0 {
                rule.timeout_seconds = default_timeout();
            }
            if rule.async_rewake {
                rule.async_background = true;
            }
            // hooks.ts::matchesPattern treats a malformed regex as a
            // non-match. A broken matcher must not prevent other user hooks
            // from running or stop the Agent from starting.
            let matcher = HookMatcher::compile(rule.matcher.as_deref());
            compiled.push((rule, matcher));
        }
        Ok(Self {
            rules: compiled,
            workspace: None,
            original_cwd: Some(std::env::current_dir().context("Capturing original hook cwd")?),
            project_root: None,
        })
    }

    /// Run user-configured hooks from the same workspace as the Agent,
    /// including nested worktree/subagent sessions.
    pub fn with_workspace(mut self, workspace: PathBuf) -> Self {
        self.workspace = Some(workspace);
        self
    }

    /// The original getProjectRoot remains stable when a subagent changes
    /// its execution cwd to a temporary worktree.
    pub fn with_project_root(mut self, project_root: PathBuf) -> Self {
        self.project_root = Some(project_root);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// FileChanged rules in registration order, for the file watcher that
    /// mirrors utils/hooks/fileChangedWatcher.ts (it re-reads the hook config
    /// snapshot to decide what to watch and which commands to run).
    pub fn file_changed_rules(&self) -> Vec<HookRule> {
        self.rules
            .iter()
            .filter(|(rule, _)| rule.event == HookEvent::FileChanged)
            .map(|(rule, _)| rule.clone())
            .collect()
    }

    pub async fn run(
        &self,
        event: HookEvent,
        tool_name: Option<&str>,
        input: Value,
    ) -> Result<HookOutcome> {
        use futures::future::join_all;

        // 2.1.88 hooks.ts executes all matching hooks concurrently with the
        // same original hookInput. A rewrite is applied to the tool only
        // after the hook results have been collected.
        // Execute concurrently but merge in registration order, as the original
        // Promise.all hook dispatcher does. Completion-time order would make
        // multiple PreCompact instructions nondeterministic.
        let mut pending = Vec::new();
        let shutdown_cap = (event.name() == "SessionEnd").then(session_end_timeout);
        let workspace = self.workspace.as_deref();
        let original_cwd = self.original_cwd.as_deref();
        let project_root = self.project_root.as_deref();
        let event_name = event.name();
        // The source chooses the matcher input from the event's own field.
        // Passing a synthetic tool name for compact or subagent hooks must
        // never add a tool_name field to their stdin JSON.
        let match_query = match event_name {
            "PreToolUse" | "PostToolUse" | "PostToolUseFailure" | "PermissionRequest"
            | "PermissionDenied" => {
                tool_name.or_else(|| input.get("tool_name").and_then(Value::as_str))
            }
            "SessionStart" => input.get("source").and_then(Value::as_str),
            "SessionEnd" => input.get("reason").and_then(Value::as_str),
            "Setup" | "PreCompact" | "PostCompact" => input.get("trigger").and_then(Value::as_str),
            "Notification" => input.get("notification_type").and_then(Value::as_str),
            "StopFailure" => input.get("error").and_then(Value::as_str),
            "SubagentStart" | "SubagentStop" => input.get("agent_type").and_then(Value::as_str),
            "Elicitation" | "ElicitationResult" => {
                input.get("mcp_server_name").and_then(Value::as_str)
            }
            "ConfigChange" => input.get("source").and_then(Value::as_str),
            "InstructionsLoaded" => input.get("load_reason").and_then(Value::as_str),
            "FileChanged" => input
                .get("file_path")
                .and_then(Value::as_str)
                .and_then(|path| Path::new(path).file_name())
                .and_then(|name| name.to_str()),
            _ => None,
        };
        // hooks.ts::getMatchingHooks deduplicates command hooks before
        // launching them. Preserve first-registration order while retaining
        // the last matching definition for an identical command.
        let mut unique_rules: Vec<&HookRule> = Vec::new();
        let mut positions =
            std::collections::HashMap::<(&str, &str, Option<&str>, Option<&Path>), usize>::new();
        for (rule, matcher) in &self.rules {
            if rule.event.name() != event_name || !matcher.matches(match_query) {
                continue;
            }
            if rule.if_condition.as_deref().is_some_and(|condition| {
                !hook_if_matches(condition, event_name, match_query, &input)
            }) {
                continue;
            }
            // hooks.ts::getMatchingHooks deduplicates commands by both
            // command text and shell; explicit bash equals the legacy default.
            let key = (
                rule.command.as_str(),
                rule.shell.unwrap_or_default().label(),
                rule.if_condition.as_deref(),
                rule.plugin_root.as_deref(),
            );
            if let Some(&position) = positions.get(&key) {
                unique_rules[position] = rule;
            } else {
                positions.insert(key, unique_rules.len());
                unique_rules.push(rule);
            }
        }
        for rule in unique_rules {
            let mut data = json!({"hook_event_name": event_name});
            if matches!(
                event_name,
                "PreToolUse"
                    | "PostToolUse"
                    | "PostToolUseFailure"
                    | "PermissionRequest"
                    | "PermissionDenied"
            ) {
                if let Some(name) = tool_name {
                    data["tool_name"] = json!(name);
                }
            }
            if let Some(fields) = input.as_object() {
                for (key, value) in fields {
                    if key != "hook_event_name" && key != "tool_name" {
                        data[key] = value.clone();
                    }
                }
            } else {
                data["input"] = input.clone();
            }
            pending.push(async move {
                let result = if let Some(cap) = shutdown_cap {
                    match timeout(
                        cap,
                        run_command(rule, &data, workspace, original_cwd, project_root),
                    )
                    .await
                    {
                        Ok(result) => result,
                        Err(_) => Err(anyhow::anyhow!("SessionEnd hook exceeded shutdown timeout")),
                    }
                } else {
                    run_command(rule, &data, workspace, original_cwd, project_root).await
                };
                (rule, result)
            });
        }

        let mut outcome = HookOutcome::default();
        for (rule, result) in join_all(pending).await {
            let response = match result {
                Ok(result) => result,
                Err(error) => {
                    outcome
                        .non_blocking_errors
                        .push(format!("Hook '{}': {error:#}", rule.command));
                    continue;
                }
            };
            if let Some(reason) = response.blocking_reason {
                if outcome.blocking_reason.is_none() {
                    outcome.blocking_reason = Some(reason.clone());
                }
                outcome.blocking_reasons.push(reason);
            }
            outcome
                .additional_context
                .extend(response.additional_context);
            outcome.watch_paths.extend(response.watch_paths);
            if let Some(message) = response.initial_user_message {
                outcome.initial_user_message = Some(message);
            }
            outcome
                .non_blocking_errors
                .extend(response.non_blocking_errors);
            if let Some(updated) = response.updated_input {
                outcome.updated_input = Some(updated);
            }
            if let Some(decision) = response.permission_decision {
                // Original hooks.ts:2820-2847: deny > ask > allow, regardless
                // of completion order or the configured hook order.
                let priority = |value: &str| match value {
                    "deny" => 3,
                    "ask" => 2,
                    "allow" => 1,
                    _ => 0,
                };
                let previous = outcome
                    .permission_decision
                    .as_deref()
                    .map(priority)
                    .unwrap_or(0);
                if priority(&decision) > previous {
                    outcome.permission_decision = Some(decision);
                }
            }
            if let Some(updated) = response.updated_mcp_tool_output {
                if event.name() == "PostToolUse" {
                    outcome.updated_mcp_tool_output = Some(updated);
                }
            }
            if response.prevent_continuation {
                outcome.prevent_continuation = true;
            }
            if let Some(reason) = response.stop_reason {
                outcome.stop_reason = Some(reason);
            }
        }
        // SessionEnd owns all still-running hook commands for this session.
        // Its 2.1.88 finalizePendingAsyncHooks cancels unfinished processes
        // before the SDK/CLI exits; never leave a detached hook alive.
        if event_name == "SessionEnd" {
            if let Some(session_id) = input.get("session_id").and_then(Value::as_str) {
                for notification in finalize_async_hooks(session_id).await {
                    outcome.non_blocking_errors.push(format!(
                        "Async {} hook {}",
                        notification["hook_event_name"].as_str().unwrap_or("Hook"),
                        notification["outcome"].as_str().unwrap_or("cancelled"),
                    ));
                }
            }
        }
        Ok(outcome)
    }

    pub async fn before_tool(&self, call: &ToolCall) -> Result<HookOutcome> {
        self.before_tool_with_context(call, json!({})).await
    }

    /// The active Agent supplies the current session, working directory and
    /// permission mode. This avoids caching stale context across /clear,
    /// branch, resumed sessions and SDK permission-mode changes.
    pub async fn before_tool_with_context(
        &self,
        call: &ToolCall,
        mut context: Value,
    ) -> Result<HookOutcome> {
        let arguments: Value = serde_json::from_str(&call.function.arguments)?;
        context["tool_use_id"] = json!(call.id);
        context["tool_input"] = arguments;
        self.run(HookEvent::PreToolUse, Some(&call.function.name), context)
            .await
    }

    pub async fn after_tool(&self, call: &ToolCall, output: &str) -> Result<HookOutcome> {
        self.after_tool_with_context(call, output, json!({})).await
    }

    pub async fn after_tool_with_context(
        &self,
        call: &ToolCall,
        output: &str,
        context: Value,
    ) -> Result<HookOutcome> {
        self.after_tool_with_response(call, output, json!(output), context)
            .await
    }

    pub async fn after_tool_with_response(
        &self,
        call: &ToolCall,
        output: &str,
        response: Value,
        mut context: Value,
    ) -> Result<HookOutcome> {
        let event = if output.starts_with("ERROR:") {
            HookEvent::PostToolUseFailure
        } else {
            HookEvent::PostToolUse
        };
        // Preserve the effective input after PreToolUse rewrites and attach
        // the actual tool result or failure, alongside live session context.
        let arguments: Value = serde_json::from_str(&call.function.arguments)?;
        context["tool_use_id"] = json!(call.id);
        context["tool_input"] = arguments;
        if matches!(event, HookEvent::PostToolUseFailure) {
            context["error"] = json!(output.strip_prefix("ERROR:").unwrap_or(output).trim());
            context["is_interrupt"] = json!(false);
        } else {
            context["tool_response"] = response;
        }
        self.run(event, Some(&call.function.name), context).await
    }
}

/// Validate 2.1.88 types/hooks.ts synchronous hook JSON at its runtime boundary.
/// Unknown vendor-only extensions are ignored like Zod's stripping object schema.
fn validate_hook_json(value: &Value, expected: &str) -> Result<()> {
    if !value.is_object() {
        bail!("Hook output must be an object")
    }
    // `async` / `asyncTimeout` belong to the union's async variant. When one
    // still reaches the sync validation path (e.g. pretty-printed output the
    // first-line handshake could not parse), the original falls through to the
    // non-strict sync schema: both keys are stripped as unknown and the result
    // is an empty, successful no-op (hooks.ts:2535 outcome 'success').
    for name in ["continue", "suppressOutput"] {
        if value.get(name).is_some_and(|x| !x.is_boolean()) {
            bail!("{name}: expected boolean")
        }
    }
    for name in ["stopReason", "reason", "systemMessage"] {
        if value.get(name).is_some_and(|x| !x.is_string()) {
            bail!("{name}: expected string")
        }
    }
    if let Some(decision) = value.get("decision") {
        if !matches!(decision.as_str(), Some("approve" | "block")) {
            bail!("decision: expected approve or block")
        }
    }
    let Some(out) = value.get("hookSpecificOutput") else {
        return Ok(());
    };
    let event = out
        .get("hookEventName")
        .and_then(Value::as_str)
        .context("hookSpecificOutput.hookEventName must be a known event")?;
    if event != expected {
        bail!("Hook returned incorrect event name: expected '{expected}' but got '{event}'")
    }
    let check_string = |name: &str| -> Result<()> {
        if out.get(name).is_some_and(|v| !v.is_string()) {
            bail!("{name}: expected string")
        }
        Ok(())
    };
    let check_object = |name: &str| -> Result<()> {
        if out.get(name).is_some_and(|v| !v.is_object()) {
            bail!("{name}: expected object")
        }
        Ok(())
    };
    let check_bool = |name: &str| -> Result<()> {
        if out.get(name).is_some_and(|v| !v.is_boolean()) {
            bail!("{name}: expected boolean")
        }
        Ok(())
    };
    let check_paths = || -> Result<()> {
        if let Some(paths) = out.get("watchPaths") {
            if !paths
                .as_array()
                .is_some_and(|arr| arr.iter().all(Value::is_string))
            {
                bail!("watchPaths: expected array of strings")
            }
        }
        Ok(())
    };
    match event {
        "PreToolUse" => {
            if let Some(d) = out.get("permissionDecision") {
                if !matches!(d.as_str(), Some("allow" | "deny" | "ask")) {
                    bail!("permissionDecision: expected allow, deny or ask")
                }
            }
            check_string("permissionDecisionReason")?;
            check_object("updatedInput")?;
            check_string("additionalContext")?;
        }
        "SessionStart" => {
            check_string("additionalContext")?;
            check_string("initialUserMessage")?;
            check_paths()?;
        }
        "UserPromptSubmit" | "Setup" | "SubagentStart" | "PostToolUse" | "PostToolUseFailure"
        | "Notification" => check_string("additionalContext")?,
        "PermissionDenied" => check_bool("retry")?,
        "PermissionRequest" => {
            if let Some(decision) = out.get("decision") {
                let behavior = decision
                    .get("behavior")
                    .and_then(Value::as_str)
                    .context("PermissionRequest.decision.behavior is required")?;
                match behavior {
                    "allow" => {
                        if decision.get("updatedInput").is_some_and(|v| !v.is_object()) {
                            bail!("PermissionRequest.updatedInput: expected object");
                        }
                        if decision
                            .get("updatedPermissions")
                            .is_some_and(|v| !v.is_array())
                        {
                            bail!("PermissionRequest.updatedPermissions: expected array");
                        }
                    }
                    "deny" => {
                        if decision.get("message").is_some_and(|v| !v.is_string())
                            || decision.get("interrupt").is_some_and(|v| !v.is_boolean())
                        {
                            bail!("PermissionRequest.deny: invalid fields")
                        }
                    }
                    _ => bail!("PermissionRequest.decision.behavior: invalid '{behavior}'"),
                }
            }
        }
        "Elicitation" | "ElicitationResult" => {
            if let Some(action) = out.get("action") {
                if !matches!(action.as_str(), Some("accept" | "decline" | "cancel")) {
                    bail!("Elicitation.action: invalid value");
                }
            }
            check_object("content")?;
        }
        "CwdChanged" | "FileChanged" => check_paths()?,
        "WorktreeCreate" => {
            if !out.get("worktreePath").is_some_and(Value::is_string) {
                bail!("WorktreeCreate.worktreePath: required string");
            }
        }
        _ => bail!("Unknown hook-specific event '{event}'"),
    }
    Ok(())
}

/// Plain successful command output is a visible hook attachment in 2.1.88.
/// The provider-neutral CLI delivers it to the model as local hook context
/// for the events whose results enter the ongoing conversation.
fn plain_hook_context(payload: &Value, output: &[u8]) -> Vec<String> {
    let event = payload.get("hook_event_name").and_then(Value::as_str);
    if !matches!(
        event,
        Some(
            "SessionStart"
                | "PreCompact"
                | "UserPromptSubmit"
                | "PreToolUse"
                | "PostToolUse"
                | "PostToolUseFailure"
        )
    ) {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(output);
    let text = text.trim();
    if text.is_empty() {
        Vec::new()
    } else {
        vec![text.to_owned()]
    }
}

// Original ShellCommand uses tree-kill for timeout and cancellation. A
// process-group guard prevents abandoned hook grandchildren from retaining
// pipes or continuing side effects after the parent future is cancelled.
#[cfg(unix)]
struct HookProcessGroupGuard(Option<u32>);

#[cfg(unix)]
impl Drop for HookProcessGroupGuard {
    fn drop(&mut self) {
        if let Some(pid) = self.0.filter(|pid| *pid <= i32::MAX as u32) {
            let _ = unsafe { libc::kill(-(pid as i32), libc::SIGKILL) };
        }
    }
}

/// AsyncHookRegistry.ts: after a hook reports {"async":true}, retain
/// ownership of its still-running process and deliver its final output once.
/// The registry is session-scoped; a command is never replayed automatically.
struct PendingAsyncHook {
    id: String,
    session_id: String,
    event: String,
    command: String,
    rewake: bool,
    deadline: std::time::Instant,
    task: tokio::task::JoinHandle<Result<(std::process::ExitStatus, Vec<u8>, Vec<u8>)>>,
}

fn async_hook_registry() -> &'static std::sync::Mutex<Vec<PendingAsyncHook>> {
    static REGISTRY: std::sync::OnceLock<std::sync::Mutex<Vec<PendingAsyncHook>>> =
        std::sync::OnceLock::new();
    REGISTRY.get_or_init(|| std::sync::Mutex::new(Vec::new()))
}

/// Snapshot completed hooks, releasing the global lock before any async wait.
pub async fn collect_async_hook_responses(session_id: &str) -> Vec<Value> {
    let mut completed = Vec::new();
    {
        let mut pending = async_hook_registry()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut index = 0;
        while index < pending.len() {
            if pending[index].session_id == session_id
                && (pending[index].task.is_finished()
                    || std::time::Instant::now() >= pending[index].deadline)
            {
                completed.push(pending.remove(index));
            } else {
                index += 1;
            }
        }
    }
    let mut responses = Vec::new();
    for mut hook in completed {
        let timed_out = std::time::Instant::now() >= hook.deadline && !hook.task.is_finished();
        if timed_out {
            hook.task.abort();
        }
        let finished = hook.task.await;
        let (outcome, exit_code, stdout, stderr) = match finished {
            Ok(Ok((status, out, err))) => (
                if status.success() { "success" } else { "error" },
                status.code(),
                String::from_utf8_lossy(&out).into_owned(),
                String::from_utf8_lossy(&err).into_owned(),
            ),
            Ok(Err(error)) => ("error", None, String::new(), format!("{error:#}")),
            Err(error) if timed_out || error.is_cancelled() => (
                "cancelled",
                None,
                String::new(),
                "Async hook cancelled or timed out".to_owned(),
            ),
            Err(error) => ("error", None, String::new(), format!("{error:#}")),
        };
        // The initial async JSON is a handshake; only later JSON lines are
        // accepted as the final synchronous hook response.
        let response = stdout
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line.trim()).ok())
            .find(|value| value.is_object() && value.get("async").is_none())
            .unwrap_or_else(|| json!({}));
        responses.push(json!({
            "type":"hook_response","hook_id":hook.id,"hook_event_name":hook.event,
            "command":hook.command,"outcome":outcome,"exit_code":exit_code,
            "stdout":stdout,"stderr":stderr,"response":response,
            "rewake":hook.rewake && exit_code == Some(2)
        }));
    }
    responses
}

/// Mirror finalizePendingAsyncHooks on SDK/REPL teardown: cancel any
/// outstanding processes and wait for their cleanup without reusing tokens.
pub async fn finalize_async_hooks(session_id: &str) -> Vec<Value> {
    let mut removed = Vec::new();
    {
        let mut pending = async_hook_registry()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut index = 0;
        while index < pending.len() {
            if pending[index].session_id == session_id {
                removed.push(pending.remove(index));
            } else {
                index += 1;
            }
        }
    }
    let mut events = Vec::new();
    for hook in removed {
        hook.task.abort();
        let _ = hook.task.await;
        events.push(json!({"type":"hook_response","hook_id":hook.id,
            "hook_event_name":hook.event,"command":hook.command,
            "outcome":"cancelled"}));
    }
    events
}

struct HookTaskDropGuard {
    handle: tokio::task::AbortHandle,
    transferred: bool,
}
impl Drop for HookTaskDropGuard {
    fn drop(&mut self) {
        if !self.transferred {
            self.handle.abort();
        }
    }
}

/// SDK interrupt and SessionEnd cancellation must also terminate child
/// processes spawned by a PowerShell/Git Bash hook on Windows.
#[cfg(windows)]
struct HookCancelTreeGuard(Option<u32>);

#[cfg(windows)]
impl Drop for HookCancelTreeGuard {
    fn drop(&mut self) {
        if let Some(pid) = self.0 {
            let _ = std::process::Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/T", "/F"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
}

async fn run_command(
    rule: &HookRule,
    payload: &Value,
    workspace: Option<&Path>,
    original_cwd: Option<&Path>,
    project_root: Option<&Path>,
) -> Result<HookOutcome> {
    // A plugin can be removed while the session is active. Check before
    // launching so a missing script does not accidentally exit with the
    // protocol's blocking status 2. Keep each plugin root isolated.
    let command_text = if let Some(plugin_root) = rule.plugin_root.as_deref() {
        if !plugin_root.is_dir() {
            bail!(
                "Plugin hook directory is no longer available: {}",
                plugin_root.display()
            );
        }
        #[cfg(windows)]
        let root_text = if rule.shell.unwrap_or_default() == crate::shell_runtime::ShellKind::Bash {
            crate::portable_platform::windows_path_to_posix_path(&plugin_root.to_string_lossy())
        } else {
            plugin_root.to_string_lossy().into_owned()
        };
        #[cfg(not(windows))]
        let root_text = plugin_root.to_string_lossy().into_owned();
        rule.command
            .replace("${ATLAS_PLUGIN_ROOT}", &root_text)
            .replace("${ATLAS_PLUGIN_ROOT}", &root_text)
    } else {
        rule.command.clone()
    };
    let mut proc = match rule.shell.unwrap_or_default() {
        crate::shell_runtime::ShellKind::Powershell => {
            // A per-hook PowerShell script runs with its own native argv,
            // independent of the Agent's default shell.
            crate::shell_runtime::ShellKind::Powershell.build_command(&command_text)?
        }
        crate::shell_runtime::ShellKind::Bash => {
            #[cfg(windows)]
            {
                // The source uses Git Bash on Windows, including .sh scripts.
                let trimmed = command_text.trim();
                let shell_script = trimmed.ends_with(".sh")
                    || trimmed.contains(".sh ")
                    || trimmed.contains(".sh\"");
                let command = if shell_script && !trimmed.starts_with("bash ") {
                    format!("bash {}", command_text)
                } else {
                    command_text.clone()
                };
                crate::shell_runtime::ShellKind::Bash.build_command(&command)?
            }
            #[cfg(not(windows))]
            {
                let mut cmd = Command::new("sh");
                cmd.arg("-c").arg(&command_text);
                cmd
            }
        }
    };
    proc.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    crate::subprocess_env::apply(&mut proc);
    if let Some(plugin_root) = rule.plugin_root.as_deref() {
        #[cfg(windows)]
        let env_root = if rule.shell.unwrap_or_default() == crate::shell_runtime::ShellKind::Bash {
            crate::portable_platform::windows_path_to_posix_path(&plugin_root.to_string_lossy())
        } else {
            plugin_root.to_string_lossy().into_owned()
        };
        #[cfg(not(windows))]
        let env_root = plugin_root.to_string_lossy().into_owned();
        proc.env("ATLAS_PLUGIN_ROOT", env_root);
    }
    if let Some(root) = workspace {
        // hooks.ts::execCommandHook uses the original cwd when a worktree has
        // already been removed. Keep the stable project-root environment value
        // while launching the cleanup hook from a directory that still exists.
        let safe_cwd = if root.is_dir() {
            root
        } else {
            original_cwd
                .filter(|path| path.is_dir())
                .context("Hook workspace was removed and original cwd is unavailable")?
        };
        proc.current_dir(safe_cwd);
        let stable_root = project_root.unwrap_or(root);
        #[cfg(windows)]
        {
            let project_dir = if rule.shell.unwrap_or_default()
                == crate::shell_runtime::ShellKind::Bash
            {
                crate::portable_platform::windows_path_to_posix_path(&stable_root.to_string_lossy())
            } else {
                stable_root.to_string_lossy().into_owned()
            };
            proc.env("ATLAS_PROJECT_DIR", project_dir);
        }
        #[cfg(not(windows))]
        proc.env("ATLAS_PROJECT_DIR", stable_root);
    }
    // Each hook owns a fresh process group, so cancellation never kills
    // unrelated terminal processes or other concurrent hooks.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(proc.as_std_mut(), 0);
    let mut child = proc
        .spawn()
        .with_context(|| format!("Starting hook '{}'", rule.command))?;
    #[cfg(unix)]
    let mut group_guard = HookProcessGroupGuard(child.id());
    #[cfg(windows)]
    let mut windows_tree_guard = HookCancelTreeGuard(child.id());
    let mut stdin = child.stdin.take().context("Hook stdin unavailable")?;
    let mut stdout = child.stdout.take().context("Hook stdout unavailable")?;
    let mut stderr = child.stderr.take().context("Hook stderr unavailable")?;
    // The original writes JSON followed by a newline. Shell hooks using
    // `read -r line` must see a complete line before stdin is closed.
    let mut input = serde_json::to_vec(payload)?;
    input.push(b'\n');
    if input.len() > 256_000 {
        bail!("Hook input exceeds 256 KB")
    }
    // hooks.ts detects the first response line before the hook process
    // completes. A JSON {"async":true} transfers process ownership to the
    // registry; ordinary hooks continue waiting for the complete output.
    let (first_sender, first_receiver) = tokio::sync::oneshot::channel::<Vec<u8>>();
    let operation = async move {
        let input_future = async move {
            if let Err(error) = stdin.write_all(&input).await {
                if error.kind() != std::io::ErrorKind::BrokenPipe {
                    return Err(anyhow::Error::from(error));
                }
            }
            let _ = stdin.shutdown().await;
            drop(stdin);
            Ok::<_, anyhow::Error>(())
        };
        let output_future = async move {
            use tokio::io::AsyncBufReadExt;
            let mut buffered = tokio::io::BufReader::new(stdout);
            let mut first = Vec::new();
            loop {
                let chunk = buffered.fill_buf().await?;
                if chunk.is_empty() {
                    break;
                }
                let n = chunk
                    .iter()
                    .position(|byte| *byte == b'\n')
                    .map(|index| index + 1)
                    .unwrap_or(chunk.len());
                if first.len().saturating_add(n) > 64_000 {
                    bail!("Hook output exceeds 64000 bytes");
                }
                let found_newline = chunk[..n].contains(&b'\n');
                first.extend_from_slice(&chunk[..n]);
                buffered.consume(n);
                if found_newline {
                    break;
                }
            }
            let _ = first_sender.send(first.clone());
            let mut remaining =
                read_limited(&mut buffered, 64_000usize.saturating_sub(first.len())).await?;
            first.append(&mut remaining);
            Ok::<_, anyhow::Error>(first)
        };
        let errors_future = read_limited(&mut stderr, 16_000);
        let (_, out, err) = tokio::try_join!(input_future, output_future, errors_future)?;
        let status = child.wait().await?;
        #[cfg(unix)]
        {
            group_guard.0 = None;
        }
        #[cfg(windows)]
        {
            windows_tree_guard.0 = None;
        }
        Ok::<_, anyhow::Error>((status, out, err))
    };
    let task = tokio::spawn(operation);
    let mut drop_guard = HookTaskDropGuard {
        handle: task.abort_handle(),
        transferred: false,
    };
    let started = tokio::time::Instant::now();
    let sync_deadline = started + Duration::from_secs(rule.timeout_seconds);
    if rule.async_background {
        let session_id = payload
            .get("session_id")
            .and_then(Value::as_str)
            .context("Background hook requires session_id")?
            .to_owned();
        let mut pending = async_hook_registry()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if pending.len() >= 128 {
            bail!("Too many pending asynchronous hooks (maximum 128)");
        }
        pending.push(PendingAsyncHook {
            id: uuid::Uuid::new_v4().to_string(),
            session_id,
            event: payload["hook_event_name"].as_str().unwrap_or("").to_owned(),
            command: rule.command.clone(),
            rewake: rule.async_rewake,
            deadline: std::time::Instant::now() + Duration::from_secs(rule.timeout_seconds),
            task,
        });
        drop_guard.transferred = true;
        return Ok(HookOutcome::default());
    }
    let first_line = tokio::time::timeout_at(sync_deadline, first_receiver)
        .await
        .context("Hook timed out before first output")?
        .context("Hook exited before output could be inspected")?;
    if let Ok(hello) = serde_json::from_slice::<Value>(&first_line) {
        if hello.get("async") == Some(&Value::Bool(true)) {
            let timeout_ms = hello
                .get("asyncTimeout")
                .map(|value| {
                    value
                        .as_u64()
                        .filter(|v| *v > 0)
                        .context("asyncTimeout must be a positive millisecond count")
                })
                .transpose()?
                .unwrap_or(15_000);
            let session_id = payload
                .get("session_id")
                .and_then(Value::as_str)
                .context("Asynchronous hook requires session_id")?
                .to_owned();
            let mut pending = async_hook_registry()
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if pending.len() >= 128 {
                bail!("Too many pending asynchronous hooks (maximum 128)");
            }
            pending.push(PendingAsyncHook {
                id: uuid::Uuid::new_v4().to_string(),
                session_id,
                event: payload["hook_event_name"].as_str().unwrap_or("").to_owned(),
                command: rule.command.clone(),
                rewake: false,
                deadline: std::time::Instant::now() + Duration::from_millis(timeout_ms),
                task,
            });
            drop_guard.transferred = true;
            // The first JSON line is an asynchronous acknowledgement. It is
            // not permission approval or a completed hook response.
            return Ok(HookOutcome::default());
        }
    }
    let (status, out, err) = tokio::time::timeout_at(sync_deadline, task)
        .await
        .context("Hook timed out")?
        .context("Hook worker was cancelled")??;
    let stderr_text = String::from_utf8_lossy(&err).trim().to_string();
    // Original hooks.ts:2499-2533, 2543-2613, 2647-2695 parses JSON
    // before interpreting the exit status. Valid JSON decisions apply even
    // when a command exits nonzero; plain-text status 2 alone blocks.
    if !String::from_utf8_lossy(&out).trim_start().starts_with('{') {
        if status.code() == Some(2) {
            return Ok(HookOutcome {
                blocking_reason: Some(if stderr_text.is_empty() {
                    "Hook blocked this operation".into()
                } else {
                    stderr_text
                }),
                ..Default::default()
            });
        }
        if !status.success() {
            return Ok(HookOutcome {
                non_blocking_errors: vec![format!(
                    "Hook '{}' exited {status}: {}",
                    rule.command,
                    if stderr_text.is_empty() {
                        "No stderr output"
                    } else {
                        &stderr_text
                    }
                )],
                ..Default::default()
            });
        }
        return Ok(HookOutcome {
            additional_context: plain_hook_context(payload, &out),
            ..Default::default()
        });
    }
    let value: Value = match serde_json::from_slice(&out) {
        Ok(value) => value,
        Err(error) => {
            // Syntax errors fall back to plain-text exit-code handling in
            // hooks.ts:399-450; only a schema validation error is reported
            // separately. Exit code 2 must still block in this case.
            if status.code() == Some(2) {
                return Ok(HookOutcome {
                    blocking_reason: Some(if stderr_text.is_empty() {
                        "Hook blocked this operation".into()
                    } else {
                        stderr_text
                    }),
                    ..Default::default()
                });
            }
            if status.success() {
                return Ok(HookOutcome {
                    additional_context: plain_hook_context(payload, &out),
                    ..Default::default()
                });
            }
            return Ok(HookOutcome {
                non_blocking_errors: vec![format!(
                    "Hook '{}' returned invalid JSON: {error}",
                    rule.command
                )],
                ..Default::default()
            });
        }
    };
    // A syntactically valid JSON object is not necessarily a valid hook output.
    // The original types/hooks.ts uses a discriminated Zod union; validation
    // failure yields a non-blocking hook error, even with exit status 2.
    if let Err(error) =
        validate_hook_json(&value, payload["hook_event_name"].as_str().unwrap_or(""))
    {
        return Ok(HookOutcome {
            non_blocking_errors: vec![format!(
                "Hook '{}' JSON validation failed: {error:#}",
                rule.command
            )],
            ..Default::default()
        });
    }
    let mut result = HookOutcome::default();
    // 2.1.88 emits systemMessage as a model-visible hook attachment.
    // Preserve it through the existing Agent hook-context delivery path.
    if let Some(message) = value.get("systemMessage").and_then(Value::as_str) {
        if !message.is_empty() {
            result.additional_context.push(message.to_owned());
        }
    }
    let decision = value.get("decision").and_then(Value::as_str);
    let hook_output = value.get("hookSpecificOutput");
    // Original 2.1.88 hooks.ts:581-589 rejects outputs whose
    // hookEventName is missing or mismatched. A response for another
    // lifecycle event must not rewrite this tool's inputs or permissions.
    if let Some(output) = hook_output.filter(|item| !item.is_null()) {
        let expected = payload
            .get("hook_event_name")
            .and_then(Value::as_str)
            .context("Hook input missing hook_event_name")?;
        let actual = output.get("hookEventName").and_then(Value::as_str);
        if actual != Some(expected) {
            bail!(
                "Hook returned incorrect event name: expected '{expected}', got '{}'",
                actual.unwrap_or("<missing>")
            );
        }
    }
    let explicit_decision = hook_output
        .and_then(|x| x.get("permissionDecision"))
        .and_then(Value::as_str);
    // The source parser rejects unknown decision variants. Silently ignoring
    // one could turn a malformed denial into an approval under --approval full.
    if let Some(value) = explicit_decision {
        if !matches!(value, "allow" | "deny" | "ask") {
            bail!("Unknown hook permissionDecision type: {value}");
        }
    }
    let legacy_decision = match decision {
        Some("approve") => Some("allow"),
        Some("block") => Some("deny"),
        Some(other) => bail!("Unknown hook decision type: {other}"),
        None => None,
    };
    let permission_decision = explicit_decision.or(legacy_decision);
    if decision == Some("block") || permission_decision == Some("deny") {
        result.blocking_reason = Some(
            hook_output
                .and_then(|v| v.get("permissionDecisionReason"))
                .or_else(|| value.get("reason"))
                .and_then(Value::as_str)
                .unwrap_or("Hook blocked operation")
                .to_string(),
        );
    }
    if value.get("continue").and_then(Value::as_bool) == Some(false) {
        result.prevent_continuation = true;
        // A sync hook's continue:false is not an implicit tool denial.
        // Stop/agent callers decide how to handle prevent_continuation.
    }
    result.stop_reason = value
        .get("stopReason")
        .and_then(Value::as_str)
        .map(str::to_owned);
    if let Some(context) = hook_output
        .and_then(|v| v.get("additionalContext"))
        .or_else(|| value.get("additionalContext"))
        .and_then(Value::as_str)
    {
        result.additional_context.push(context.into());
    }
    if payload["hook_event_name"] == "PreToolUse" {
        result.permission_decision = permission_decision.map(str::to_owned);
    }
    if payload["hook_event_name"] == "SessionStart" {
        result.initial_user_message = hook_output
            .and_then(|v| v.get("initialUserMessage"))
            .and_then(Value::as_str)
            .filter(|message| !message.trim().is_empty())
            .map(str::to_owned);
    }
    if matches!(
        payload["hook_event_name"].as_str(),
        Some("FileChanged" | "CwdChanged")
    ) {
        if let Some(paths) = hook_output
            .and_then(|v| v.get("watchPaths"))
            .and_then(Value::as_array)
        {
            result.watch_paths = paths
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect();
        }
    }
    if let Some(input) = hook_output.and_then(|v| v.get("updatedInput")) {
        if !input.is_object() {
            bail!("updatedInput must be a JSON object")
        }
        result.updated_input = Some(input.clone());
    }
    if payload["hook_event_name"] == "PostToolUse" {
        result.updated_mcp_tool_output = hook_output
            .and_then(|v| v.get("updatedMCPToolOutput"))
            .cloned();
    }
    Ok(result)
}

async fn read_limited<R: tokio::io::AsyncRead + Unpin>(
    reader: &mut R,
    max: usize,
) -> Result<Vec<u8>> {
    let mut buffer = [0u8; 8192];
    let mut result = Vec::new();
    loop {
        let count = reader.read(&mut buffer).await?;
        if count == 0 {
            return Ok(result);
        }
        if result.len() + count > max {
            bail!("Hook output exceeds {max} bytes")
        }
        result.extend_from_slice(&buffer[..count]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[tokio::test]
    async fn file_changed_hook_returns_dynamic_watch_paths_in_registration_order() {
        let hooks = HookRunner::new(&[HookRule {
            async_background: false,
            async_rewake: false,
            event: HookEvent::FileChanged,
            matcher: None,
            if_condition: None,
            plugin_root: None,
            command: r#"printf '%s\n' '{"hookSpecificOutput":{"hookEventName":"FileChanged","watchPaths":["generated/config.json",".env"]}}'"#.into(),
            shell: None,
            timeout_seconds: 3,
        }])
        .unwrap();
        let outcome = hooks
            .run(
                HookEvent::FileChanged,
                None,
                json!({"file_path":"/project/.env","event":"change"}),
            )
            .await
            .unwrap();
        assert!(outcome.non_blocking_errors.is_empty(), "{outcome:?}");
        assert_eq!(outcome.watch_paths, vec!["generated/config.json", ".env"]);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn subagent_matcher_receives_type_and_flattened_agent_identity() {
        let hook = HookRunner::new(&[
            HookRule {
                async_background: false,
                async_rewake: false,
                if_condition: None,
                plugin_root: None,
                event: HookEvent::SubagentStart,
                matcher: Some("^explore$".into()),
                command: "cat | grep -q '\"agent_id\":\"worker-1\"'".into(),
                shell: None,
                timeout_seconds: 3,
            },
            HookRule {
                async_background: false,
                async_rewake: false,
                if_condition: None,
                plugin_root: None,
                event: HookEvent::SubagentStart,
                matcher: Some("^plan$".into()),
                command: "echo 'wrong hook' >&2; exit 2".into(),
                shell: None,
                timeout_seconds: 3,
            },
        ])
        .unwrap();
        let out = hook
            .run(
                HookEvent::SubagentStart,
                Some("explore"),
                json!({"agent_id":"worker-1","agent_type":"explore"}),
            )
            .await
            .unwrap();
        assert!(out.blocking_reason.is_none());
        let skipped = hook
            .run(
                HookEvent::SubagentStart,
                Some("general"),
                json!({"agent_id":"worker-1","agent_type":"general"}),
            )
            .await
            .unwrap();
        assert!(skipped.blocking_reason.is_none());
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn subagent_stop_blocking_does_not_mutate_prior_tool_output() {
        let hook = HookRunner::new(&[HookRule {
            async_background: false,
            async_rewake: false,
            if_condition: None,
            plugin_root: None,
            event: HookEvent::SubagentStop,
            matcher: Some("^general$".into()),
            command: "echo 'review not complete' >&2; exit 2".into(),
            shell: None,
            timeout_seconds: 3,
        }])
        .unwrap();
        let result = hook
            .run(
                HookEvent::SubagentStop,
                Some("general"),
                json!({"agent_id":"worker-1","agent_type":"general",
                "last_assistant_message":"File was edited"}),
            )
            .await
            .unwrap();
        assert_eq!(
            result.blocking_reason.as_deref(),
            Some("review not complete")
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn post_tool_mcp_output_is_replaced_before_next_hook() {
        let runner=HookRunner::new(&[
            HookRule{ async_background: false, async_rewake: false,  if_condition: None, plugin_root: None,event:HookEvent::PostToolUse,
                matcher:Some("^mcp__mock__secret$".into()),
                command:r#"printf '{"hookSpecificOutput":{"hookEventName":"PostToolUse","updatedMCPToolOutput":{"content":[{"type":"text","text":"REDACTED_MCP_OUTPUT"}]}}}'"#.into(),
                shell: None, timeout_seconds:3},
            HookRule{ async_background: false, async_rewake: false,  if_condition: None, plugin_root: None,event:HookEvent::PostToolUse,
                matcher:Some("^mcp__mock__secret$".into()),
                command:"cat | grep -q REDACTED_MCP_OUTPUT".into(),
                shell: None, timeout_seconds:3},
        ]).unwrap();
        let result = runner
            .run(
                HookEvent::PostToolUse,
                Some("mcp__mock__secret"),
                json!({"tool_response":"SENSITIVE_MCP_OUTPUT"}),
            )
            .await
            .unwrap();
        assert_eq!(
            result.updated_mcp_tool_output,
            Some(json!({"content":[{"type":"text","text":"REDACTED_MCP_OUTPUT"}]}))
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn wrong_hook_specific_event_is_a_protocol_error_not_a_permission_grant() {
        for json_output in [
            r#"{"hookSpecificOutput":{"hookEventName":"PostToolUse","permissionDecision":"allow"}}"#,
            r#"{"hookSpecificOutput":{"permissionDecision":"allow"}}"#,
        ] {
            let runner = HookRunner::new(&[HookRule {
                async_background: false,
                async_rewake: false,
                if_condition: None,
                plugin_root: None,
                event: HookEvent::PreToolUse,
                matcher: None,
                command: format!("printf '%s' '{}'", json_output),
                shell: None,
                timeout_seconds: 3,
            }])
            .unwrap();
            let outcome = runner
                .run(HookEvent::PreToolUse, None, json!({}))
                .await
                .unwrap();
            assert!(outcome.blocking_reason.is_none());
            assert_eq!(outcome.non_blocking_errors.len(), 1);
            assert!(
                outcome.non_blocking_errors[0].contains("incorrect event name")
                    || outcome.non_blocking_errors[0]
                        .contains("hookEventName must be a known event")
            );
        }
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn legacy_approve_and_invalid_permission_variants_match_source_contract() {
        let approved = HookRunner::new(&[HookRule {
            async_background: false,
            async_rewake: false,
            if_condition: None,
            plugin_root: None,
            event: HookEvent::PreToolUse,
            matcher: None,
            command: "printf '{\"decision\":\"approve\"}'".into(),
            shell: None,
            timeout_seconds: 3,
        }])
        .unwrap();
        let outcome = approved
            .run(HookEvent::PreToolUse, None, json!({}))
            .await
            .unwrap();
        assert_eq!(outcome.permission_decision.as_deref(), Some("allow"));

        for json_output in [
            r#"{"decision":"unrestricted"}"#,
            r#"{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"unrestricted"}}"#,
        ] {
            let runner = HookRunner::new(&[HookRule {
                async_background: false,
                async_rewake: false,
                if_condition: None,
                plugin_root: None,
                event: HookEvent::PreToolUse,
                matcher: None,
                command: format!("printf '%s' '{}'", json_output),
                shell: None,
                timeout_seconds: 3,
            }])
            .unwrap();
            let outcome = runner
                .run(HookEvent::PreToolUse, None, json!({}))
                .await
                .unwrap();
            assert!(outcome.blocking_reason.is_none());
            assert_eq!(outcome.non_blocking_errors.len(), 1);
            assert!(outcome.non_blocking_errors[0].contains("validation failed"));
        }
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn schema_failure_is_advisory_and_next_hook_still_changes_real_tool_input() {
        use crate::{
            protocol::{FunctionCall, ToolCall},
            tools::{Approval, ToolExecutor},
        };
        let root = tempfile::tempdir().unwrap();
        let runner=HookRunner::new(&[
            HookRule { async_background: false, async_rewake: false,  if_condition: None, plugin_root: None,event:HookEvent::PreToolUse,matcher:None,
                command:r#"printf '{"decision":"approve","continue":"no"}'; exit 2"#.into(),shell: None, timeout_seconds:3},
            HookRule { async_background: false, async_rewake: false,  if_condition: None, plugin_root: None,event:HookEvent::PreToolUse,matcher:None,
                command:r#"printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","updatedInput":{"command":"touch hook_schema_ok"}}}'"#.into(),shell: None, timeout_seconds:3},
        ]).unwrap();
        let mut call = ToolCall {
            id: "schema".into(),
            kind: "function".into(),
            function: FunctionCall {
                name: "run_command".into(),
                arguments: json!({"command":"touch old_command"}).to_string(),
            },
        };
        let outcome = runner.before_tool(&call).await.unwrap();
        assert!(outcome.blocking_reason.is_none());
        assert_eq!(outcome.non_blocking_errors.len(), 1);
        assert!(outcome.non_blocking_errors[0].contains("continue: expected boolean"));
        call.function.arguments = outcome.updated_input.unwrap().to_string();
        let tool = ToolExecutor::new(root.path().canonicalize().unwrap(), Approval::Full, false);
        let result = tool.execute(&call).await;
        assert!(result.contains("Exit:"), "{result}");
        assert!(root.path().join("hook_schema_ok").exists());
        assert!(!root.path().join("old_command").exists());
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn continuation_flag_does_not_become_permission_denial() {
        let runner = HookRunner::new(&[HookRule {
            async_background: false,
            async_rewake: false,
            if_condition: None,
            plugin_root: None,
            event: HookEvent::PreToolUse,
            matcher: None,
            command: r#"printf '{"continue":false,"stopReason":"stop requested"}'"#.into(),
            shell: None,
            timeout_seconds: 3,
        }])
        .unwrap();
        let result = runner
            .run(HookEvent::PreToolUse, None, json!({}))
            .await
            .unwrap();
        assert!(result.prevent_continuation);
        assert!(result.blocking_reason.is_none());
        assert!(result.permission_decision.is_none());
    }
    #[test]
    fn async_variant_output_reaching_sync_validation_is_a_stripped_noop() {
        // types/hooks.ts:171-177 — the hookJSONOutputSchema union validates the
        // async variant; hooks.ts:2535 then yields plain 'success'. When the
        // first-line handshake misses the acknowledgement (pretty-printed
        // JSON), the sync path must not error: unknown union keys are stripped
        // and every remaining field is absent, so the outcome is a no-op.
        for output in [
            json!({"async": true}),
            json!({"async": true, "asyncTimeout": 30_000}),
            // Non-literal async values fail the async schema but are stripped
            // by the non-strict sync schema exactly like any unknown key.
            json!({"async": false}),
            json!({"async": "true"}),
        ] {
            validate_hook_json(&output, "PreToolUse")
                .unwrap_or_else(|error| panic!("{output}: {error:#}"));
        }
    }
    #[test]
    fn agent_tool_hooks_match_the_reachable_rust_spawn_entrypoints() {
        // Agent::ask calls HookRunner::before_tool_with_context before
        // dispatching spawn_agent/spawn_team; source matchers use "Agent".
        for name in ["spawn_agent", "spawn_team"] {
            assert!(HookMatcher::compile(Some("Agent")).matches(Some(name)));
            assert!(HookMatcher::compile(Some("^Agent$")).matches(Some(name)));
            assert!(HookMatcher::compile(Some("Task")).matches(Some(name)));
            assert!(HookMatcher::compile(Some("^Task$")).matches(Some(name)));
        }
    }
    #[test]
    fn legacy_hook_matchers_resolve_current_task_tools() {
        for (old, current) in [
            ("Task", "Agent"),
            ("KillShell", "task_stop"),
            ("AgentOutputTool", "task_output"),
            ("BashOutputTool", "task_output"),
        ] {
            assert!(HookMatcher::compile(Some(old)).matches(Some(current)));
            let regex = format!("^{old}$");
            assert!(HookMatcher::compile(Some(&regex)).matches(Some(current)));
        }
    }
    #[test]
    fn hook_shell_configuration_preserves_explicit_powershell_and_legacy_bash() {
        let ps: HookRule = serde_json::from_value(json!({
            "event": "PreToolUse",
            "command": "Write-Output ready",
            "shell": "powershell",
            "timeout_seconds": 3
        }))
        .unwrap();
        assert_eq!(ps.shell, Some(crate::shell_runtime::ShellKind::Powershell));
        let legacy: HookRule = serde_json::from_value(json!({
            "event": "PreToolUse",
            "command": "echo ready"
        }))
        .unwrap();
        assert_eq!(legacy.shell, None);
        assert_eq!(legacy.shell.unwrap_or_default().label(), "bash");
        assert_eq!(legacy.timeout_seconds, 600);
        assert!(serde_json::to_value(&legacy)
            .unwrap()
            .get("shell")
            .is_none());
    }
    #[test]
    fn validates_hook_configuration() {
        let invalid = HookRunner::new(&[HookRule {
            async_background: false,
            async_rewake: false,
            if_condition: None,
            plugin_root: None,
            event: HookEvent::PreToolUse,
            matcher: Some("[".into()),
            command: "true".into(),
            shell: None,
            timeout_seconds: 20,
        }])
        .unwrap();
        assert!(matches!(&invalid.rules[0].1, HookMatcher::Invalid));
        assert!(!invalid.rules[0].1.matches(Some("Bash")));
        let zero = HookRunner::new(&[HookRule {
            async_background: false,
            async_rewake: false,
            if_condition: None,
            plugin_root: None,
            event: HookEvent::Stop,
            matcher: None,
            command: "true".into(),
            shell: None,
            timeout_seconds: 0,
        }])
        .unwrap();
        assert_eq!(zero.rules[0].0.timeout_seconds, 600);
        let extended = HookRunner::new(&[HookRule {
            async_background: false,
            async_rewake: false,
            if_condition: None,
            plugin_root: None,
            event: HookEvent::Stop,
            matcher: None,
            command: "true".into(),
            shell: None,
            timeout_seconds: 3_600,
        }])
        .unwrap();
        assert_eq!(extended.rules[0].0.timeout_seconds, 3_600);
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn blocks_tool_and_rewrites_input() {
        let blocker = HookRunner::new(&[HookRule {
            async_background: false,
            async_rewake: false,
            if_condition: None,
            plugin_root: None,
            event: HookEvent::PreToolUse,
            matcher: Some("^run_command$".into()),
            command: "printf '{\"decision\":\"block\",\"reason\":\"policy\"}'".into(),
            shell: None,
            timeout_seconds: 5,
        }])
        .unwrap();
        let call = ToolCall {
            id: "abc".into(),
            kind: "function".into(),
            function: crate::protocol::FunctionCall {
                name: "run_command".into(),
                arguments: json!({"command":"echo hello"}).to_string(),
            },
        };
        assert_eq!(
            blocker
                .before_tool(&call)
                .await
                .unwrap()
                .blocking_reason
                .as_deref(),
            Some("policy")
        );
        let updater = HookRunner::new(&[HookRule { async_background: false, async_rewake: false,  if_condition: None, plugin_root: None,
            event: HookEvent::PreToolUse,
            matcher: None,
            command: "printf '{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"updatedInput\":{\"path\":\"safe.txt\"}}}'"
                .into(),
            shell: None, timeout_seconds: 5,
        }])
        .unwrap();
        let result = updater.before_tool(&call).await.unwrap();
        assert_eq!(result.updated_input.unwrap()["path"], "safe.txt");
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn exit_code_two_blocks() {
        let hooks = HookRunner::new(&[HookRule {
            async_background: false,
            async_rewake: false,
            if_condition: None,
            plugin_root: None,
            event: HookEvent::Stop,
            matcher: None,
            command: "echo 'Need tests' >&2; exit 2".into(),
            shell: None,
            timeout_seconds: 5,
        }])
        .unwrap();
        let result = hooks.run(HookEvent::Stop, None, json!({})).await.unwrap();
        assert_eq!(result.blocking_reason.as_deref(), Some("Need tests"));
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn nonblocking_hook_failure_preserves_later_input_rewrite_and_real_shell_execution() {
        use crate::{
            protocol::{FunctionCall, ToolCall},
            tools::{Approval, ToolExecutor},
        };
        let root = tempfile::tempdir().unwrap();
        let runner = HookRunner::new(&[
            HookRule { async_background: false, async_rewake: false,  if_condition: None, plugin_root: None,
                event: HookEvent::PreToolUse,
                matcher: Some("^run_command$".into()),
                command: "echo 'advisory hook failed' >&2; exit 1".into(),
                shell: None, timeout_seconds: 3,
            },
            HookRule { async_background: false, async_rewake: false,  if_condition: None, plugin_root: None,
                event: HookEvent::PreToolUse,
                matcher: Some("^run_command$".into()),
                command: r#"printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","updatedInput":{"command":"touch after_warning"}}}'"#.into(),
                shell: None, timeout_seconds: 3,
            },
        ])
        .unwrap();
        let mut call = ToolCall {
            id: "hook-nonblocking-1".into(),
            kind: "function".into(),
            function: FunctionCall {
                name: "run_command".into(),
                arguments: json!({"command":"touch before_warning"}).to_string(),
            },
        };
        let outcome = runner.before_tool(&call).await.unwrap();
        assert!(outcome.blocking_reason.is_none());
        assert_eq!(outcome.non_blocking_errors.len(), 1);
        assert!(outcome.non_blocking_errors[0].contains("advisory hook failed"));
        call.function.arguments = outcome.updated_input.unwrap().to_string();
        let executor =
            ToolExecutor::new(root.path().canonicalize().unwrap(), Approval::Full, false);
        let result = executor.execute(&call).await;
        assert!(result.contains("Exit: exit status: 0"), "{result}");
        assert!(root.path().join("after_warning").exists());
        assert!(!root.path().join("before_warning").exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn pretool_hooks_observe_original_input_like_the_source() {
        let runner = HookRunner::new(&[
            HookRule { async_background: false, async_rewake: false,  if_condition: None, plugin_root: None,
                event: HookEvent::PreToolUse,
                matcher: None,
                command: r#"printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","updatedInput":{"command":"rm sensitive"}}}'"#.into(),
                shell: None, timeout_seconds: 3,
            },
            HookRule { async_background: false, async_rewake: false,  if_condition: None, plugin_root: None,
                event: HookEvent::PreToolUse,
                matcher: None,
                command: r#"cat | grep -q 'echo safe' && { echo 'original input observed' >&2; exit 2; } || { echo 'unexpected rewritten input' >&2; exit 2; }"#.into(),
                shell: None, timeout_seconds: 3,
            },
        ]).unwrap();
        let call = ToolCall {
            id: "rewrite-check".into(),
            kind: "function".into(),
            function: crate::protocol::FunctionCall {
                name: "run_command".into(),
                arguments: json!({"command":"echo safe"}).to_string(),
            },
        };
        let outcome = runner.before_tool(&call).await.unwrap();
        assert_eq!(
            outcome.blocking_reason.as_deref(),
            Some("original input observed")
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn matching_command_hooks_run_concurrently_with_original_input() {
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("first-ready");
        let second = root.path().join("second-ready");
        let command = |own: &std::path::Path, peer: &std::path::Path| {
            format!(
                "touch '{}'; i=0; while [ \"$i\" -lt 100 ]; do test -e '{}' && exit 0; i=$((i+1)); sleep 0.02; done; echo hooks ran sequentially >&2; exit 2",
                own.display(),
                peer.display()
            )
        };
        let runner = HookRunner::new(&[
            HookRule {
                async_background: false,
                async_rewake: false,
                if_condition: None,
                plugin_root: None,
                event: HookEvent::PreToolUse,
                matcher: None,
                command: command(&first, &second),
                shell: None,
                timeout_seconds: 5,
            },
            HookRule {
                async_background: false,
                async_rewake: false,
                if_condition: None,
                plugin_root: None,
                event: HookEvent::PreToolUse,
                matcher: None,
                command: command(&second, &first),
                shell: None,
                timeout_seconds: 5,
            },
        ])
        .unwrap();
        let result = runner
            .run(HookEvent::PreToolUse, Some("run_command"), json!({}))
            .await
            .unwrap();
        assert!(result.blocking_reason.is_none(), "{result:?}");
        assert!(first.exists() && second.exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn hook_permission_decisions_keep_deny_over_ask_over_allow() {
        let runner = HookRunner::new(&[
            HookRule { async_background: false, async_rewake: false,  if_condition: None, plugin_root: None,
                event: HookEvent::PreToolUse,
                matcher: None,
                command: r#"printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"allow"}}'"#.into(),
                shell: None, timeout_seconds: 3,
            },
            HookRule { async_background: false, async_rewake: false,  if_condition: None, plugin_root: None,
                event: HookEvent::PreToolUse,
                matcher: None,
                command: r#"printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"ask"}}'"#.into(),
                shell: None, timeout_seconds: 3,
            },
            HookRule { async_background: false, async_rewake: false,  if_condition: None, plugin_root: None,
                event: HookEvent::PreToolUse,
                matcher: None,
                command: r#"printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny"}}'"#.into(),
                shell: None, timeout_seconds: 3,
            },
        ])
        .unwrap();
        let result = runner
            .run(HookEvent::PreToolUse, Some("run_command"), json!({}))
            .await
            .unwrap();
        assert_eq!(result.permission_decision.as_deref(), Some("deny"));
        assert!(result.blocking_reason.is_some());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn command_hook_json_precedes_exit_status_and_invalid_json_is_nonblocking() {
        let approved = HookRunner::new(&[HookRule {
            async_background: false,
            async_rewake: false,
            if_condition: None,
            plugin_root: None,
            event: HookEvent::PreToolUse,
            matcher: None,
            command: "printf '{\"decision\":\"approve\"}'; exit 2".into(),
            shell: None,
            timeout_seconds: 3,
        }])
        .unwrap();
        let outcome = approved
            .run(HookEvent::PreToolUse, None, json!({}))
            .await
            .unwrap();
        assert!(outcome.blocking_reason.is_none());
        assert_eq!(outcome.permission_decision.as_deref(), Some("allow"));

        let malformed = HookRunner::new(&[
            HookRule {
                async_background: false,
                async_rewake: false,
                if_condition: None,
                plugin_root: None,
                event: HookEvent::PreToolUse,
                matcher: None,
                command: "printf '{not-json'; exit 1".into(),
                shell: None,
                timeout_seconds: 3,
            },
            HookRule {
                async_background: false,
                async_rewake: false,
                if_condition: None,
                plugin_root: None,
                event: HookEvent::PreToolUse,
                matcher: None,
                command: "printf '{\"decision\":\"approve\"}'".into(),
                shell: None,
                timeout_seconds: 3,
            },
        ])
        .unwrap();
        let outcome = malformed
            .run(HookEvent::PreToolUse, None, json!({}))
            .await
            .unwrap();
        assert!(outcome.blocking_reason.is_none());
        assert_eq!(outcome.permission_decision.as_deref(), Some("allow"));
        assert_eq!(outcome.non_blocking_errors.len(), 1);
        assert!(outcome.non_blocking_errors[0].contains("invalid JSON"));

        let malformed_block = HookRunner::new(&[HookRule {
            async_background: false,
            async_rewake: false,
            if_condition: None,
            plugin_root: None,
            event: HookEvent::PreToolUse,
            matcher: None,
            command: "printf '{not-json'; echo 'blocked syntax' >&2; exit 2".into(),
            shell: None,
            timeout_seconds: 3,
        }])
        .unwrap();
        assert_eq!(
            malformed_block
                .run(HookEvent::PreToolUse, None, json!({}))
                .await
                .unwrap()
                .blocking_reason
                .as_deref(),
            Some("blocked syntax")
        );

        let blocked = HookRunner::new(&[HookRule {
            async_background: false,
            async_rewake: false,
            if_condition: None,
            plugin_root: None,
            event: HookEvent::PreToolUse,
            matcher: None,
            command: "echo 'explicit denial' >&2; exit 2".into(),
            shell: None,
            timeout_seconds: 3,
        }])
        .unwrap();
        assert_eq!(
            blocked
                .run(HookEvent::PreToolUse, None, json!({}))
                .await
                .unwrap()
                .blocking_reason
                .as_deref(),
            Some("explicit denial")
        );
    }
}
