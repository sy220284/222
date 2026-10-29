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
    UserPromptSubmit,
    PreToolUse,
    PostToolUse,
    PostToolUseFailure,
    Stop,
    PreCompact,
    PostCompact,
    TaskCreated,
    TaskCompleted,
}

impl HookEvent {
    pub fn name(&self) -> &'static str {
        match self {
            Self::SessionStart => "SessionStart",
            Self::SessionEnd => "SessionEnd",
            Self::UserPromptSubmit => "UserPromptSubmit",
            Self::PreToolUse => "PreToolUse",
            Self::PostToolUse => "PostToolUse",
            Self::PostToolUseFailure => "PostToolUseFailure",
            Self::Stop => "Stop",
            Self::PreCompact => "PreCompact",
            Self::PostCompact => "PostCompact",
            Self::TaskCreated => "TaskCreated",
            Self::TaskCompleted => "TaskCompleted",
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

fn default_timeout() -> u64 { 30 }

#[derive(Default, Debug, Clone)]
pub struct HookOutcome {
    pub blocking_reason: Option<String>,
    pub additional_context: Vec<String>,
    pub updated_input: Option<Value>,
    pub prevent_continuation: bool,
}

#[derive(Default)]
pub struct HookRunner {
    rules: Vec<(HookRule, Option<Regex>)>,
}

impl HookRunner {
    pub fn new(rules: &[HookRule]) -> Result<Self> {
        let mut compiled = Vec::with_capacity(rules.len());
        for rule in rules {
            if rule.command.trim().is_empty() { bail!("Hook command cannot be empty"); }
            if !(1..=600).contains(&rule.timeout_seconds) {
                bail!("Hook timeout must be between 1 and 600 seconds");
            }
            let matcher = rule.matcher.as_deref().map(Regex::new).transpose()?;
            compiled.push((rule.clone(), matcher));
        }
        Ok(Self { rules: compiled })
    }

    pub fn is_empty(&self) -> bool { self.rules.is_empty() }

    pub async fn run(&self, event: HookEvent, tool_name: Option<&str>, input: Value) -> Result<HookOutcome> {
        let mut outcome = HookOutcome::default();
        for (rule, matcher) in &self.rules {
            if rule.event.name() != event.name() { continue; }
            if let Some(pattern) = matcher {
                if !pattern.is_match(tool_name.unwrap_or("")) { continue; }
            }
            // Original hook stdin fields are top-level, not nested in input.
            let mut data = json!({"hook_event_name": event.name()});
            if let Some(name) = tool_name { data["tool_name"] = json!(name); }
            if let Some(fields) = input.as_object() {
                for (key, value) in fields {
                    if key != "hook_event_name" && key != "tool_name" {
                        data[key] = value.clone();
                    }
                }
            } else {
                data["input"] = input.clone();
            }
            let response = run_command(rule, &data).await?;
            if response.blocking_reason.is_some() {
                return Ok(response);
            }
            outcome.additional_context.extend(response.additional_context);
            if let Some(updated) = response.updated_input {
                outcome.updated_input = Some(updated);
            }
            if response.prevent_continuation {
                outcome.prevent_continuation = true;
                break;
            }
        }
        Ok(outcome)
    }

    pub async fn before_tool(&self, call: &ToolCall) -> Result<HookOutcome> {
        let arguments: Value = serde_json::from_str(&call.function.arguments)?;
        self.run(HookEvent::PreToolUse, Some(&call.function.name),
            json!({"tool_use_id":call.id, "tool_input":arguments})).await
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
    proc.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = proc.spawn()
        .with_context(|| format!("Starting hook '{}'", rule.command))?;
    let mut stdin = child.stdin.take().context("Hook stdin unavailable")?;
    let mut stdout = child.stdout.take().context("Hook stdout unavailable")?;
    let mut stderr = child.stderr.take().context("Hook stderr unavailable")?;
    let input = serde_json::to_vec(payload)?;
    if input.len() > 256_000 { bail!("Hook input exceeds 256 KB") }
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
        .await.context("Hook timed out")??;
    let stderr_text = String::from_utf8_lossy(&err).trim().to_string();
    if status.code() == Some(2) {
        return Ok(HookOutcome {
            blocking_reason: Some(if stderr_text.is_empty() {
                "Hook blocked this operation".into()
            } else { stderr_text }),
            ..Default::default()
        });
    }
    if !status.success() {
        bail!("Hook exited {}: {}", status, stderr_text);
    }
    if out.iter().all(|b| b.is_ascii_whitespace()) {
        return Ok(HookOutcome::default());
    }
    let value: Value = serde_json::from_slice(&out)
        .context("Hook stdout must be valid JSON")?;
    let mut result = HookOutcome::default();
    let decision = value.get("decision").and_then(Value::as_str);
    let hook_output = value.get("hookSpecificOutput");
    let permission_decision = hook_output.and_then(|x| x.get("permissionDecision"))
        .and_then(Value::as_str);
    if decision == Some("block") || permission_decision == Some("deny") {
        result.blocking_reason = Some(
            hook_output.and_then(|v| v.get("permissionDecisionReason"))
                .or_else(|| value.get("reason"))
                .and_then(Value::as_str).unwrap_or("Hook blocked operation").to_string());
    }
    if value.get("continue").and_then(Value::as_bool) == Some(false) {
        result.prevent_continuation = true;
        result.blocking_reason.get_or_insert_with(|| value.get("stopReason")
            .and_then(Value::as_str).unwrap_or("Hook stopped continuation").into());
    }
    if let Some(context) = hook_output.and_then(|v| v.get("additionalContext"))
        .or_else(|| value.get("additionalContext")).and_then(Value::as_str) {
        result.additional_context.push(context.into());
    }
    if let Some(input) = hook_output.and_then(|v| v.get("updatedInput")) {
        if !input.is_object() { bail!("updatedInput must be a JSON object") }
        result.updated_input = Some(input.clone());
    }
    Ok(result)
}

async fn read_limited<R: tokio::io::AsyncRead + Unpin>(reader: &mut R, max: usize) -> Result<Vec<u8>> {
    let mut buffer = [0u8; 8192];
    let mut result = Vec::new();
    loop {
        let count = reader.read(&mut buffer).await?;
        if count == 0 { return Ok(result) }
        if result.len() + count > max { bail!("Hook output exceeds {max} bytes") }
        result.extend_from_slice(&buffer[..count]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_hook_configuration() {
        assert!(HookRunner::new(&[HookRule {
            event: HookEvent::PreToolUse, matcher: Some("[".into()),
            command: "true".into(), timeout_seconds: 20,
        }]).is_err());
        assert!(HookRunner::new(&[HookRule {
            event: HookEvent::Stop, matcher: None,
            command: "true".into(), timeout_seconds: 0,
        }]).is_err());
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn blocks_tool_and_rewrites_input() {
        let blocker = HookRunner::new(&[HookRule {
            event: HookEvent::PreToolUse, matcher: Some("^run_command$".into()),
            command: "printf '{\"decision\":\"block\",\"reason\":\"policy\"}'".into(),
            timeout_seconds: 5,
        }]).unwrap();
        let call = ToolCall {
            id:"abc".into(), kind:"function".into(),
            function:crate::protocol::FunctionCall {
                name:"run_command".into(),
                arguments:json!({"command":"echo hello"}).to_string()
            }
        };
        assert_eq!(blocker.before_tool(&call).await.unwrap().blocking_reason.as_deref(), Some("policy"));
        let updater = HookRunner::new(&[HookRule {
            event: HookEvent::PreToolUse, matcher: None,
            command: "printf '{\"hookSpecificOutput\":{\"updatedInput\":{\"path\":\"safe.txt\"}}}'".into(),
            timeout_seconds: 5,
        }]).unwrap();
        let result = updater.before_tool(&call).await.unwrap();
        assert_eq!(result.updated_input.unwrap()["path"], "safe.txt");
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn exit_code_two_blocks() {
        let hooks = HookRunner::new(&[HookRule {
            event: HookEvent::Stop, matcher: None,
            command: "echo 'Need tests' >&2; exit 2".into(),
            timeout_seconds: 5,
        }]).unwrap();
        let result = hooks.run(HookEvent::Stop, None, json!({})).await.unwrap();
        assert_eq!(result.blocking_reason.as_deref(), Some("Need tests"));
    }
}
