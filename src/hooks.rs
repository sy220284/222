use crate::protocol::ToolCall;
use anyhow::{bail, Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{process::Stdio, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
    time::timeout,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
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
    pub event: HookEvent,
    /// Regular expression against tool name for tool events.
    #[serde(default)]
    pub matcher: Option<String>,
    /// User-configured local command. Never downloaded or installed automatically.
    pub command: String,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
}

fn default_timeout() -> u64 {
    30
}

#[derive(Default, Debug, Clone)]
pub struct HookOutcome {
    pub blocking_reason: Option<String>,
    pub additional_context: Vec<String>,
    pub updated_input: Option<Value>,
    /// PreToolUse hook decision. An explicit `ask` requires user approval
    /// even when the CLI normally grants full tool access.
    pub permission_decision: Option<String>,
    /// Replacement for an MCP tool's model-visible output from PostToolUse.
    pub updated_mcp_tool_output: Option<Value>,
    pub prevent_continuation: bool,
    /// Non-blocking command-hook failures remain visible without denying the tool.
    pub non_blocking_errors: Vec<String>,
}

#[derive(Default)]
pub struct HookRunner {
    rules: Vec<(HookRule, Option<Regex>)>,
}

impl HookRunner {
    pub fn new(rules: &[HookRule]) -> Result<Self> {
        let mut compiled = Vec::with_capacity(rules.len());
        for rule in rules {
            if rule.command.trim().is_empty() {
                bail!("Hook command cannot be empty");
            }
            if !(1..=600).contains(&rule.timeout_seconds) {
                bail!("Hook timeout must be between 1 and 600 seconds");
            }
            let matcher = rule.matcher.as_deref().map(Regex::new).transpose()?;
            compiled.push((rule.clone(), matcher));
        }
        Ok(Self { rules: compiled })
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    pub async fn run(
        &self,
        event: HookEvent,
        tool_name: Option<&str>,
        input: Value,
    ) -> Result<HookOutcome> {
        use futures::{stream::FuturesUnordered, StreamExt};

        // 2.1.88 hooks.ts executes all matching hooks concurrently with the
        // same original hookInput. A rewrite is applied to the tool only
        // after the hook results have been collected.
        let mut pending = FuturesUnordered::new();
        for (rule, matcher) in &self.rules {
            if rule.event.name() != event.name() {
                continue;
            }
            if let Some(pattern) = matcher {
                if !pattern.is_match(tool_name.unwrap_or("")) {
                    continue;
                }
            }
            let mut data = json!({"hook_event_name": event.name()});
            if let Some(name) = tool_name {
                data["tool_name"] = json!(name);
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
            pending.push(async move { (rule, run_command(rule, &data).await) });
        }

        let mut outcome = HookOutcome::default();
        while let Some((rule, result)) = pending.next().await {
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
                    outcome.blocking_reason = Some(reason);
                }
            }
            outcome
                .additional_context
                .extend(response.additional_context);
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
        }
        Ok(outcome)
    }

    pub async fn before_tool(&self, call: &ToolCall) -> Result<HookOutcome> {
        let arguments: Value = serde_json::from_str(&call.function.arguments)?;
        self.run(
            HookEvent::PreToolUse,
            Some(&call.function.name),
            json!({"tool_use_id":call.id, "tool_input":arguments}),
        )
        .await
    }

    pub async fn after_tool(&self, call: &ToolCall, output: &str) -> Result<HookOutcome> {
        let event = if output.starts_with("ERROR:") {
            HookEvent::PostToolUseFailure
        } else {
            HookEvent::PostToolUse
        };
        // The hook protocol carries the effective tool input as a JSON
        // object, not an escaped JSON string. PreToolUse may rewrite it.
        let arguments: Value = serde_json::from_str(&call.function.arguments)?;
        let mut payload = json!({"tool_use_id":call.id, "tool_input":arguments});
        if matches!(event, HookEvent::PostToolUseFailure) {
            payload["error"] = json!(output.strip_prefix("ERROR:").unwrap_or(output).trim());
            payload["is_interrupt"] = json!(false);
        } else {
            payload["tool_response"] = json!(output);
        }
        self.run(event, Some(&call.function.name), payload).await
    }
}

/// Validate 2.1.88 types/hooks.ts synchronous hook JSON at its runtime boundary.
/// Unknown vendor-only extensions are ignored like Zod's stripping object schema.
fn validate_hook_json(value: &Value, expected: &str) -> Result<()> {
    if !value.is_object() {
        bail!("Hook output must be an object")
    }
    if let Some(async_field) = value.get("async") {
        if async_field != true {
            bail!("async must be literal true");
        }
        // Do not misrepresent an asynchronous hook as completed. Its process
        // registration, wakeup and timeout must be implemented separately.
        bail!("Async hook registry is not yet implemented");
    }
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

async fn run_command(rule: &HookRule, payload: &Value) -> Result<HookOutcome> {
    #[cfg(windows)]
    let mut proc = {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg(&rule.command);
        cmd
    };
    #[cfg(not(windows))]
    let mut proc = {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(&rule.command);
        cmd
    };
    proc.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    crate::subprocess_env::apply(&mut proc);
    let mut child = proc
        .spawn()
        .with_context(|| format!("Starting hook '{}'", rule.command))?;
    let mut stdin = child.stdin.take().context("Hook stdin unavailable")?;
    let mut stdout = child.stdout.take().context("Hook stdout unavailable")?;
    let mut stderr = child.stderr.take().context("Hook stderr unavailable")?;
    let input = serde_json::to_vec(payload)?;
    if input.len() > 256_000 {
        bail!("Hook input exceeds 256 KB")
    }
    let operation = async {
        let input_future = async move {
            // Move the pipe into the writer future so it is dropped as soon
            // as writing finishes. A child such as 'cat' waits for EOF;
            // shutdown() alone does not reliably close ChildStdin here.
            // A hook may finish without reading stdin. Closed pipe is not
            // a hook failure; the exit code and response decide the result.
            if let Err(error) = stdin.write_all(&input).await {
                if error.kind() != std::io::ErrorKind::BrokenPipe {
                    return Err(error.into());
                }
            }
            let _ = stdin.shutdown().await;
            drop(stdin);
            Ok::<_, anyhow::Error>(())
        };
        let output_future = read_limited(&mut stdout, 64_000);
        let errors_future = read_limited(&mut stderr, 16_000);
        let (_, out, err) = tokio::try_join!(input_future, output_future, errors_future)?;
        let status = child.wait().await?;
        Ok::<_, anyhow::Error>((status, out, err))
    };
    let (status, out, err) = timeout(Duration::from_secs(rule.timeout_seconds), operation)
        .await
        .context("Hook timed out")??;
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
        return Ok(HookOutcome::default());
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
                return Ok(HookOutcome::default());
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
    async fn subagent_matcher_receives_type_and_flattened_agent_identity() {
        let hook = HookRunner::new(&[
            HookRule {
                event: HookEvent::SubagentStart,
                matcher: Some("^explore$".into()),
                command: "cat | grep -q '\"agent_id\":\"worker-1\"'".into(),
                timeout_seconds: 3,
            },
            HookRule {
                event: HookEvent::SubagentStart,
                matcher: Some("^plan$".into()),
                command: "echo 'wrong hook' >&2; exit 2".into(),
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
            event: HookEvent::SubagentStop,
            matcher: Some("^general$".into()),
            command: "echo 'review not complete' >&2; exit 2".into(),
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
            HookRule{event:HookEvent::PostToolUse,
                matcher:Some("^mcp__mock__secret$".into()),
                command:r#"printf '{"hookSpecificOutput":{"hookEventName":"PostToolUse","updatedMCPToolOutput":{"content":[{"type":"text","text":"REDACTED_MCP_OUTPUT"}]}}}'"#.into(),
                timeout_seconds:3},
            HookRule{event:HookEvent::PostToolUse,
                matcher:Some("^mcp__mock__secret$".into()),
                command:"cat | grep -q REDACTED_MCP_OUTPUT".into(),
                timeout_seconds:3},
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
                event: HookEvent::PreToolUse,
                matcher: None,
                command: format!("printf '%s' '{}'", json_output),
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
            event: HookEvent::PreToolUse,
            matcher: None,
            command: "printf '{\"decision\":\"approve\"}'".into(),
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
                event: HookEvent::PreToolUse,
                matcher: None,
                command: format!("printf '%s' '{}'", json_output),
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
            HookRule {event:HookEvent::PreToolUse,matcher:None,
                command:r#"printf '{"decision":"approve","continue":"no"}'; exit 2"#.into(),timeout_seconds:3},
            HookRule {event:HookEvent::PreToolUse,matcher:None,
                command:r#"printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","updatedInput":{"command":"touch hook_schema_ok"}}}'"#.into(),timeout_seconds:3},
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
            event: HookEvent::PreToolUse,
            matcher: None,
            command: r#"printf '{"continue":false,"stopReason":"stop requested"}'"#.into(),
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
    fn validates_hook_configuration() {
        assert!(HookRunner::new(&[HookRule {
            event: HookEvent::PreToolUse,
            matcher: Some("[".into()),
            command: "true".into(),
            timeout_seconds: 20,
        }])
        .is_err());
        assert!(HookRunner::new(&[HookRule {
            event: HookEvent::Stop,
            matcher: None,
            command: "true".into(),
            timeout_seconds: 0,
        }])
        .is_err());
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn blocks_tool_and_rewrites_input() {
        let blocker = HookRunner::new(&[HookRule {
            event: HookEvent::PreToolUse,
            matcher: Some("^run_command$".into()),
            command: "printf '{\"decision\":\"block\",\"reason\":\"policy\"}'".into(),
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
        let updater = HookRunner::new(&[HookRule {
            event: HookEvent::PreToolUse,
            matcher: None,
            command: "printf '{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"updatedInput\":{\"path\":\"safe.txt\"}}}'"
                .into(),
            timeout_seconds: 5,
        }])
        .unwrap();
        let result = updater.before_tool(&call).await.unwrap();
        assert_eq!(result.updated_input.unwrap()["path"], "safe.txt");
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn exit_code_two_blocks() {
        let hooks = HookRunner::new(&[HookRule {
            event: HookEvent::Stop,
            matcher: None,
            command: "echo 'Need tests' >&2; exit 2".into(),
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
            HookRule {
                event: HookEvent::PreToolUse,
                matcher: Some("^run_command$".into()),
                command: "echo 'advisory hook failed' >&2; exit 1".into(),
                timeout_seconds: 3,
            },
            HookRule {
                event: HookEvent::PreToolUse,
                matcher: Some("^run_command$".into()),
                command: r#"printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","updatedInput":{"command":"touch after_warning"}}}'"#.into(),
                timeout_seconds: 3,
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
            HookRule {
                event: HookEvent::PreToolUse,
                matcher: None,
                command: r#"printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","updatedInput":{"command":"rm sensitive"}}}'"#.into(),
                timeout_seconds: 3,
            },
            HookRule {
                event: HookEvent::PreToolUse,
                matcher: None,
                command: r#"cat | grep -q 'echo safe' && { echo 'original input observed' >&2; exit 2; } || { echo 'unexpected rewritten input' >&2; exit 2; }"#.into(),
                timeout_seconds: 3,
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
                event: HookEvent::PreToolUse,
                matcher: None,
                command: command(&first, &second),
                timeout_seconds: 5,
            },
            HookRule {
                event: HookEvent::PreToolUse,
                matcher: None,
                command: command(&second, &first),
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
            HookRule {
                event: HookEvent::PreToolUse,
                matcher: None,
                command: r#"printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"allow"}}'"#.into(),
                timeout_seconds: 3,
            },
            HookRule {
                event: HookEvent::PreToolUse,
                matcher: None,
                command: r#"printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"ask"}}'"#.into(),
                timeout_seconds: 3,
            },
            HookRule {
                event: HookEvent::PreToolUse,
                matcher: None,
                command: r#"printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny"}}'"#.into(),
                timeout_seconds: 3,
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
            event: HookEvent::PreToolUse,
            matcher: None,
            command: "printf '{\"decision\":\"approve\"}'; exit 2".into(),
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
                event: HookEvent::PreToolUse,
                matcher: None,
                command: "printf '{not-json'; exit 1".into(),
                timeout_seconds: 3,
            },
            HookRule {
                event: HookEvent::PreToolUse,
                matcher: None,
                command: "printf '{\"decision\":\"approve\"}'".into(),
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
            event: HookEvent::PreToolUse,
            matcher: None,
            command: "printf '{not-json'; echo 'blocked syntax' >&2; exit 2".into(),
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
            event: HookEvent::PreToolUse,
            matcher: None,
            command: "echo 'explicit denial' >&2; exit 2".into(),
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
