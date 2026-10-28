use crate::{
    config::ProviderProfile,
    protocol::{Message, ModelRequest},
    provider::{self, ModelProvider},
    session::Session,
    mcp::McpManager,
    config::McpServerConfig,
    tools::{self, Approval, ToolExecutor},
};
use anyhow::{bail, Context, Result};
use std::{fs, path::{Path, PathBuf}};

const DEFAULT_SYSTEM: &str = r#"You are Atlas, a terminal-based software engineering assistant.
You can inspect, search and modify files and execute commands using the provided tools.
Work step by step. Use actual tool results as evidence; never claim a command ran unless it did.
Before editing, inspect relevant files. Keep edits minimal and verify them when possible.
The filesystem tools are scoped to the active workspace. Shell commands require user authorization
and may affect the wider machine. Do not assume authorization.
If a tool fails, analyze its error and make a concrete correction or explain the blocker.
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
        Ok(Self { profile, provider, root, approval, interactive, max_steps, session, mcp: None })
    }

    pub async fn attach_mcp(&mut self, servers: &[McpServerConfig]) -> Result<()> {
        if !servers.is_empty() {
            self.mcp = Some(McpManager::connect(servers).await?);
        }
        Ok(())
    }

    pub fn profile(&self) -> &ProviderProfile { &self.profile }
    pub fn session_id(&self) -> String { self.session.id.to_string() }

    pub fn switch_model(&mut self, profile: ProviderProfile) -> Result<()> {
        let new_provider = provider::create(&profile)?;
        self.provider = new_provider;
        self.profile = profile;
        Ok(())
    }

    pub fn clear_history(&mut self) -> Result<()> {
        self.session.clear();
        self.session.save(&self.root)
    }

    pub async fn ask(&mut self, prompt: &str) -> Result<String> {
        if prompt.trim().is_empty() { bail!("Prompt cannot be empty") }
        self.session.messages.push(Message::text("user", prompt));
        self.session.save(&self.root)?;
        let system = system_prompt(&self.root)?;
        let executor = ToolExecutor::new(self.root.clone(), self.approval, self.interactive);
        for _ in 0..self.max_steps {
            let mut available_tools = tools::definitions();
            if let Some(mcp) = &self.mcp {
                available_tools.extend(mcp.definitions());
            }
            let response = self.provider.complete(&ModelRequest {
                model: self.profile.model.clone(),
                system: system.clone(),
                messages: self.session.messages.clone(),
                tools: available_tools,
            }).await?;
            let calls = response.message.tool_calls.clone().unwrap_or_default();
            let text = response.message.content.clone().unwrap_or_default();
            let finish = response.finish_reason.clone();
            self.session.messages.push(response.message);
            self.session.save(&self.root)?;
            if calls.is_empty() {
                if matches!(finish.as_deref(), Some("length" | "MAX_TOKENS")) {
                    bail!("Model reached its output limit before completion; session saved")
                }
                return Ok(text);
            }
            if !text.trim().is_empty() {
                eprintln!("{text}");
            }
            for call in calls {
                eprintln!("Executing tool: {}", call.function.name);
                let result = if let Some(mcp) = &mut self.mcp {
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
    #[test]
    fn loads_project_instruction_file() {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join("AGENTS.md"), "Use lowercase commands.").unwrap();
        let prompt = system_prompt(d.path()).unwrap();
        assert!(prompt.contains("Use lowercase commands."));
        assert!(prompt.contains("Current workspace:"));
    }
}
