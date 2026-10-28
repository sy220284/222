use crate::{
    config::ProviderProfile,
    protocol::{Message, ModelRequest},
    provider::{self, ModelProvider},
    session::Session,
    mcp::McpManager,
    lsp::LspManager,
    task,
    hooks::{HookRule, HookRunner, HookEvent},
    worktree::Worktree,
    protocol::ToolCall,
    config::{McpServerConfig, LspServerConfig},
    tools::{self, Approval, ToolExecutor},
};
use anyhow::{bail, Context, Result};
use std::{fs, path::{Path, PathBuf}};
use serde_json::json;

const DEFAULT_SYSTEM: &str = r#"You are Atlas, a terminal-based software engineering assistant.
You can inspect, search and modify files and execute commands using the provided tools.
Work step by step. Use actual tool results as evidence; never claim a command ran unless it did.
Before editing, inspect relevant files. Keep edits minimal and verify them when possible.
The filesystem tools are scoped to the active workspace. Shell commands require user authorization
and may affect the wider machine. Do not assume authorization.
If a tool fails, analyze its error and make a concrete correction or explain the blocker.
Use task tools to track multi-step work; mark tasks completed only when verified.
Project skills can be discovered with list_skills and loaded with read_skill when relevant.
Never reveal secrets from configuration, environment variables, or private files.
When finished, report the verified result and any remaining limitations accurately."#;

pub struct Agent {
    profile: ProviderProfile,
    provider: Box<dyn ModelProvider>,
    root: PathBuf,
    approval: Approval,
    interactive: bool,
    max_steps: usize,
    session: Session,
    mcp: Option<McpManager>,
    lsp: Option<LspManager>,
    hooks: HookRunner,
    session_context: String,
    child_depth: u8,
    mcp_configs: Vec<McpServerConfig>,
    lsp_configs: Vec<LspServerConfig>,
    hook_rules: Vec<HookRule>,
}

impl Agent {
    pub fn new(
        profile: ProviderProfile,
        root: PathBuf,
        approval: Approval,
        interactive: bool,
        max_steps: usize,
        session: Session,
    ) -> Result<Self> {
        let provider = provider::create(&profile)?;
        Ok(Self { profile, provider, root, approval, interactive, max_steps, session, mcp: None, lsp: None, hooks: HookRunner::default(), session_context: String::new(), child_depth: 0, mcp_configs: Vec::new(), lsp_configs: Vec::new(), hook_rules: Vec::new() })
    }

    pub async fn attach_mcp(&mut self, servers: &[McpServerConfig]) -> Result<()> {
        self.mcp_configs = servers.to_vec();
        if !servers.is_empty() {
            self.mcp = Some(McpManager::connect(servers).await?);
        }
        Ok(())
    }

    pub async fn attach_hooks(&mut self, rules: &[HookRule]) -> Result<()> {
        self.hooks = HookRunner::new(rules)?;
        self.hook_rules = rules.to_vec();
        let result = self.hooks.run(HookEvent::SessionStart, None,
            json!({"session_id":self.session.id,"workspace":self.root})).await?;
        if let Some(reason) = result.blocking_reason { bail!("SessionStart blocked: {reason}") }
        self.session_context = result.additional_context.join("\n");
        Ok(())
    }

    pub async fn finish(&self) -> Result<()> {
        self.hooks.run(HookEvent::SessionEnd, None,
            json!({"session_id":self.session.id,"workspace":self.root})).await?;
        Ok(())
    }

    async fn run_subagent(&self, call: &ToolCall, executor: &ToolExecutor) -> Result<String> {
        if self.child_depth >= 1 { bail!("Subagent depth limit reached") }
        let args: serde_json::Value = serde_json::from_str(&call.function.arguments)?;
        let prompt = args.get("prompt").and_then(|v| v.as_str())
            .context("spawn_agent requires prompt")?;
        if prompt.trim().is_empty() { bail!("Subagent prompt cannot be empty") }
        let isolation = args.get("isolation").and_then(|v| v.as_str()).unwrap_or("none");
        if !matches!(isolation, "none" | "worktree") {
            bail!("Unknown isolation mode {isolation}");
        }
        if isolation == "worktree" { executor.approve_external(call)?; }
        let worktree = if isolation == "worktree" {
            Some(Worktree::create(&self.root).await?)
        } else { None };
        let child_root = worktree.as_ref().map(|w| w.path.clone())
            .unwrap_or_else(|| self.root.clone());
        let mut child_profile = self.profile.clone();
        if let Some(model) = args.get("model").and_then(|v| v.as_str()) {
            if model.trim().is_empty() { bail!("Subagent model cannot be empty"); }
            child_profile.model = model.to_string();
        }
        let mut child = Self::new(
            child_profile, child_root, self.approval, self.interactive,
            self.max_steps, Session::fresh()
        )?;
        child.child_depth = self.child_depth + 1;
        let child_id = child.session_id();
        let result = async {
            child.attach_mcp(&self.mcp_configs).await?;
            child.attach_lsp(&self.lsp_configs).await?;
            child.attach_hooks(&self.hook_rules).await?;
            // Box the nested agent future to keep the recursive async type finite.
            let response = Box::pin(child.ask(prompt)).await;
            let finalization = child.finish().await;
            let answer = response?;
            finalization?;
            Ok::<_, anyhow::Error>(answer)
        }.await;
        let isolation_result = if let Some(w) = worktree {
            Some(w.finish().await?)
        } else { None };
        let answer = result?;
        Ok(format!("Subagent session: {child_id}\nResult:\n{answer}\n{}",
            isolation_result.unwrap_or_default()))
    }

    pub async fn attach_lsp(&mut self, servers: &[LspServerConfig]) -> Result<()> {
        self.lsp_configs = servers.to_vec();
        if !servers.is_empty() {
            self.lsp = Some(LspManager::connect(&self.root,servers).await?);
        }
        Ok(())
    }

    pub fn profile(&self) -> &ProviderProfile { &self.profile }
    pub fn session_id(&self) -> String { self.session.id.to_string() }
    pub fn usage(&self) -> crate::protocol::TokenUsage { self.session.usage }

    pub fn switch_model(&mut self, profile: ProviderProfile) -> Result<()> {
        let new_provider = provider::create(&profile)?;
        self.provider = new_provider;
        self.profile = profile;
        Ok(())
    }

    /// Replace previous turns with a model-generated, persistent compact summary.
    /// This is provider-neutral and never invokes executable tools.
    pub async fn compact_history(&mut self) -> Result<()> {
        if self.session.messages.is_empty() { bail!("No history to compact") }
        let before = self.hooks.run(HookEvent::PreCompact, None,
            json!({"session_id":self.session.id, "message_count":self.session.messages.len()})).await?;
        if let Some(reason) = before.blocking_reason { bail!("PreCompact blocked: {reason}") }
        let previous = self.session.summary.as_deref().unwrap_or("");
        let prompt = format!(
            "Summarize this coding conversation for accurate future continuation. \
             Preserve user requirements, verified actions, modified paths, pending tasks, \
             critical command results, errors and next steps. Be concise and factual. \
             Do not invent completed work. Earlier summary:\n{previous}"
        );
        let response = self.provider.complete(&ModelRequest {
            model: self.profile.model.clone(),
            system: prompt,
            messages: self.session.messages.clone(),
            tools: Vec::new(),
        }).await?;
        if let Some(usage) = response.usage { self.session.usage.accumulate(usage); }
        if !response.message.tool_calls.unwrap_or_default().is_empty() {
            bail!("Compaction model unexpectedly requested tools");
        }
        let summary = response.message.content.context("Compaction returned no text")?;
        if summary.trim().is_empty() { bail!("Compaction returned empty text") }
        self.session.summary = Some(summary);
        self.session.messages.clear();
        self.session.save(&self.root)?;
        let after = self.hooks.run(HookEvent::PostCompact, None,
            json!({"session_id":self.session.id,"summary":self.session.summary})).await?;
        if let Some(reason) = after.blocking_reason { bail!("PostCompact blocked: {reason}") }
        Ok(())
    }

    pub fn clear_history(&mut self) -> Result<()> {
        self.session.clear();
        self.session.save(&self.root)
    }

    pub async fn ask(&mut self, prompt: &str) -> Result<String> {
        if prompt.trim().is_empty() { bail!("Prompt cannot be empty") }
        let history_chars: usize = self.session.messages.iter()
            .map(|m| m.content.as_deref().unwrap_or("").len()
                + m.tool_calls.as_ref().map(|c| c.iter()
                    .map(|t| t.function.arguments.len()).sum::<usize>()).unwrap_or(0))
            .sum();
        if history_chars > 100_000 {
            self.compact_history().await?;
        }
        let pre_prompt = self.hooks.run(HookEvent::UserPromptSubmit, None,
            json!({"prompt":prompt,"session_id":self.session.id})).await?;
        if let Some(reason) = pre_prompt.blocking_reason { bail!("UserPromptSubmit blocked: {reason}") }
        let prompt = if pre_prompt.additional_context.is_empty() {
            prompt.to_owned()
        } else {
            format!("{prompt}\n\n[Local hook context]\n{}", pre_prompt.additional_context.join("\n"))
        };
        self.session.messages.push(Message::text("user", prompt));
        self.session.save(&self.root)?;
        let mut system = system_prompt(&self.root)?;
        if let Some(summary) = &self.session.summary {
            system.push_str("\nVerified session continuity summary:\n");
            system.push_str(summary);
        }
        if !self.session_context.is_empty() {
            system.push_str("\nSession start hook context:\n");
            system.push_str(&self.session_context);
        }
        let executor = ToolExecutor::new(self.root.clone(), self.approval, self.interactive);
        let mut stop_hook_active = false;
        for _ in 0..self.max_steps {
            let mut available_tools = tools::definitions();
            available_tools.extend(task::definitions());
            if self.child_depth == 0 {
                available_tools.push(json!({"type":"function","function":{
                    "name":"spawn_agent",
                    "description":"Delegate a coding subtask to an independent agent session, optionally in an isolated Git worktree. This is a real separate agent run; no implicit merge.",
                    "parameters":{"type":"object","properties":{
                        "prompt":{"type":"string"},
                        "model":{"type":"string"},
                        "isolation":{"type":"string","enum":["none","worktree"]}
                    },"required":["prompt"]}
                }}));
            }
            if let Some(mcp) = &self.mcp {
                available_tools.extend(mcp.definitions());
            }
            if self.lsp.is_some(){ available_tools.push(LspManager::definition()); }
            let response = self.provider.complete(&ModelRequest {
                model: self.profile.model.clone(),
                system: system.clone(),
                messages: self.session.messages.clone(),
                tools: available_tools,
            }).await?;
            if let Some(usage) = response.usage { self.session.usage.accumulate(usage); }
            let calls = response.message.tool_calls.clone().unwrap_or_default();
            let text = response.message.content.clone().unwrap_or_default();
            let finish = response.finish_reason.clone();
            self.session.messages.push(response.message);
            self.session.save(&self.root)?;
            if calls.is_empty() {
                if matches!(finish.as_deref(), Some("length" | "MAX_TOKENS")) {
                    bail!("Model reached its output limit before completion; session saved")
                }
                if !stop_hook_active {
                    let hook = self.hooks.run(HookEvent::Stop, None,
                        json!({"session_id":self.session.id,"last_assistant_message":text,
                            "stop_hook_active":false})).await?;
                    if let Some(reason) = hook.blocking_reason {
                        stop_hook_active = true;
                        self.session.messages.push(Message::text("user",
                            format!("Stop hook blocked finishing: {reason}. Continue work and verify results.")));
                        self.session.save(&self.root)?;
                        continue;
                    }
                }
                return Ok(text);
            }
            if !text.trim().is_empty() {
                eprintln!("{text}");
            }
            for mut call in calls {
                eprintln!("Executing tool: {}", call.function.name);
                let pre = self.hooks.before_tool(&call).await?;
                if let Some(updated) = pre.updated_input {
                    call.function.arguments = serde_json::to_string(&updated)?;
                }
                let result = if let Some(reason) = pre.blocking_reason {
                    format!("ERROR: PreToolUse hook blocked: {reason}")
                } else if call.function.name == "spawn_agent" {
                    match self.run_subagent(&call, &executor).await {
                        Ok(output) => output,
                        Err(error) => format!("ERROR: {error:#}"),
                    }
                } else if call.function.name == "lsp_query" {
                    if let Some(lsp)=&mut self.lsp {
                        let input:serde_json::Value=serde_json::from_str(&call.function.arguments)?;
                        lsp.execute(&input).await
                    }else{"ERROR: No LSP server configured".into()}
                } else if task::handles(&call.function.name) {
                    task::execute(&mut self.session, &call)
                } else if let Some(mcp) = &mut self.mcp {
                    if mcp.handles(&call.function.name) {
                        match executor.approve_external(&call) {
                            Ok(()) => mcp.execute(&call).await,
                            Err(error) => format!("ERROR: {error:#}"),
                        }
                    } else {
                        executor.execute(&call).await
                    }
                } else {
                    executor.execute(&call).await
                };
                let post = self.hooks.after_tool(&call, &result).await?;
                let mut result = result;
                if let Some(reason) = post.blocking_reason {
                    result.push_str(&format!("\nPostToolUse hook blocked continuation: {reason}"));
                }
                for context in post.additional_context {
                    result.push_str(&format!("\n[Local hook context] {context}"));
                }
                self.session.messages.push(Message::tool_result(&call, result));
                self.session.save(&self.root)?;
            }
        }
        bail!("Agent exceeded {} model steps; session saved for resume", self.max_steps)
    }
}

fn system_prompt(root: &Path) -> Result<String> {
    let mut prompt = format!("{DEFAULT_SYSTEM}\nCurrent workspace: {}\n", root.display());
    for name in ["AGENTS.md", ".atlas/instructions.md"] {
        let path = root.join(name);
        if !path.is_file() { continue; }
        let real_path = path.canonicalize()?;
        if !real_path.starts_with(root) {
            bail!("Project instructions cannot point outside the workspace: {}", path.display());
        }
        let bytes = fs::read(&real_path).with_context(|| format!("Read project instructions: {}", path.display()))?;
        if bytes.len() > 50_000 {
            bail!("Project instruction file too large: {}", path.display());
        }
        let content = String::from_utf8(bytes)
            .with_context(|| format!("Project instructions must be UTF-8: {}", path.display()))?;
        prompt.push_str(&format!("\nProject instructions from {name}:\n{content}\n"));
    }
    Ok(prompt)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct SummaryModel;

    #[async_trait::async_trait]
    impl ModelProvider for SummaryModel {
        async fn complete(&self, request: &ModelRequest) -> Result<crate::protocol::ModelResponse> {
            assert!(request.tools.is_empty());
            Ok(crate::protocol::ModelResponse {
                message: Message::text("assistant", "Retain verified changes and next steps."),
                finish_reason: Some("stop".into()),
                usage: None,
            })
        }
    }

    #[tokio::test]
    async fn compacted_summary_survives_session_restore() {
        let dir = tempfile::tempdir().unwrap();
        let mut session = Session::fresh();
        session.messages.push(Message::text("user", "Check this project"));
        session.messages.push(Message::text("assistant", "Checked files"));
        let id = session.id.to_string();
        let mut agent = Agent {
            profile: ProviderProfile {
                id: "fake".into(), kind: "openai-compatible".into(),
                base_url: "http://127.0.0.1:1/v1".into(),
                model: "mock".into(), api_key_env: None,
            },
            provider: Box::new(SummaryModel),
            root: dir.path().canonicalize().unwrap(),
            approval: Approval::ReadOnly,
            interactive: false,
            max_steps: 5,
            session,
            mcp: None,
            lsp: None,
            hooks: HookRunner::default(),
            session_context: String::new(),
            child_depth: 0,
            mcp_configs: Vec::new(),
            lsp_configs: Vec::new(),
            hook_rules: Vec::new(),
        };
        agent.compact_history().await.unwrap();
        let restored = Session::load(dir.path(), &id).unwrap();
        assert!(restored.messages.is_empty());
        assert_eq!(restored.summary.as_deref(), Some("Retain verified changes and next steps."));
    }

    #[test]
    fn loads_project_instruction_file() {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join("AGENTS.md"), "Use lowercase commands.").unwrap();
        let prompt = system_prompt(d.path()).unwrap();
        assert!(prompt.contains("Use lowercase commands."));
        assert!(prompt.contains("Current workspace:"));
    }
}
