//! Provider-neutral source parity for utils/messages/systemInit.ts.
use crate::{agent::Agent, command_manifest, config::Config, tools};
use anyhow::Result;
use serde_json::{json, Value};
use uuid::Uuid;

/// An SDK protocol compatibility alias applies only at the external boundary.
/// The internal tool name and dispatch stay unchanged.
pub fn sdk_compat_tool_name(name: &str) -> &str {
    if name == "Agent" {
        "Task"
    } else {
        name
    }
}

#[derive(Debug)]
pub struct InitFields {
    pub cwd: String,
    pub session_id: String,
    pub model: String,
    pub provider: String,
    pub permission_mode: String,
    pub tools: Vec<String>,
    pub mcp_servers: Vec<Value>,
    pub slash_commands: Vec<String>,
    pub agents: Vec<String>,
    pub skills: Vec<String>,
    pub plugins: Vec<Value>,
    pub credential_source: String,
}

pub fn build(fields: InitFields) -> Value {
    json!({
        "type": "system",
        "subtype": "init",
        "cwd": fields.cwd,
        "session_id": fields.session_id,
        "tools": fields.tools.iter()
            .map(|name| sdk_compat_tool_name(name))
            .collect::<Vec<_>>(),
        "mcp_servers": fields.mcp_servers,
        "model": fields.model,
        "provider": fields.provider,
        "permissionMode": fields.permission_mode,
        "slash_commands": fields.slash_commands,
        "credentialSource": fields.credential_source,
        "betas": [],
        "agent_version": env!("CARGO_PKG_VERSION"),
        "output_style": "default",
        "agents": fields.agents,
        "skills": fields.skills,
        "plugins": fields.plugins,
        "uuid": Uuid::new_v4().to_string(),
        "usage_schema": crate::runtime_contracts::empty_usage(),
    })
}

/// SDK and REPL metadata originate from one real, permission-filtered session,
/// not an invented all-tools configuration. The cloud-only transport metadata,
/// vendor subscription state and remote messaging socket are intentionally
/// omitted under the existing project-wide exclusion policy.
pub fn build_for_agent(agent: &mut Agent, config: &Config) -> Result<Value> {
    let tool_definitions = tools::definitions_with_checkpointing(config.file_checkpointing_enabled);
    let names = tool_definitions
        .iter()
        .filter_map(|tool| tool.pointer("/function/name").and_then(Value::as_str))
        .map(str::to_owned)
        .collect();
    let mcp_servers = agent
        .mcp_statuses()?
        .into_iter()
        .filter_map(|status| {
            let name = status.get("name")?.as_str()?;
            let state = status.get("status")?.as_str()?;
            Some(json!({"name": name, "status": state}))
        })
        .collect();
    let agents = agent
        .sdk_agent_descriptors()?
        .into_iter()
        .filter_map(|entry| entry.get("name")?.as_str().map(str::to_owned))
        .collect();
    let skills = agent
        .list_user_skills()?
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect();
    let plugins = agent
        .enabled_plugins()
        .iter()
        .map(|name| json!({"name": name}))
        .collect();
    let credential_source = if agent.profile().kind == "chatgpt-plan" {
        "connected_plan"
    } else if agent.profile().api_key_env.is_some() {
        "environment"
    } else {
        "none"
    };
    Ok(build(InitFields {
        cwd: agent.workspace_root().to_string_lossy().into_owned(),
        session_id: agent.session_id(),
        model: agent.profile().model.clone(),
        provider: agent.profile().id.clone(),
        permission_mode: agent.current_permission_mode().to_owned(),
        tools: names,
        mcp_servers,
        slash_commands: command_manifest::visible()
            .map(|command| command.name.to_owned())
            .collect(),
        agents,
        skills,
        plugins,
        credential_source: credential_source.to_owned(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> InitFields {
        InitFields {
            cwd: "/tmp/workspace".to_owned(),
            session_id: "session-1".to_owned(),
            model: "model-1".to_owned(),
            provider: "mock".to_owned(),
            permission_mode: "default".to_owned(),
            tools: vec!["Agent".into(), "Read".into()],
            mcp_servers: vec![json!({"name":"example","status":"connected"})],
            slash_commands: vec!["resume".into(), "memory".into()],
            agents: vec!["reviewer".into()],
            skills: vec!["notes".into()],
            plugins: vec![json!({"name":"local-plugin"})],
            credential_source: "environment".into(),
        }
    }

    #[test]
    fn sdk_system_init_preserves_source_schema_and_legacy_tool_alias() {
        let message = build(sample());
        assert_eq!(message["type"], "system");
        assert_eq!(message["subtype"], "init");
        assert_eq!(message["cwd"], "/tmp/workspace");
        assert_eq!(message["session_id"], "session-1");
        assert_eq!(message["tools"], json!(["Task", "Read"]));
        assert_eq!(message["mcp_servers"][0]["name"], "example");
        assert_eq!(message["permissionMode"], "default");
        assert_eq!(message["slash_commands"], json!(["resume", "memory"]));
        assert_eq!(message["agents"], json!(["reviewer"]));
        assert_eq!(message["skills"], json!(["notes"]));
        assert_eq!(message["plugins"][0]["name"], "local-plugin");
        assert_eq!(message["credentialSource"], "environment");
        assert!(Uuid::parse_str(message["uuid"].as_str().unwrap()).is_ok());
        assert!(message.get("usage_schema").is_some());
        assert!(message.get("apiKey").is_none());
    }

    #[test]
    fn every_init_message_gets_its_own_uuid_but_keeps_session_id() {
        let first = build(sample());
        let second = build(sample());
        assert_ne!(first["uuid"], second["uuid"]);
        assert_eq!(first["session_id"], second["session_id"]);
    }
}
