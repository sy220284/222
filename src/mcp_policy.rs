use crate::config::McpServerConfig;
use anyhow::{bail, Context, Result};
use serde_json::Value;

/// SDK-injected MCP processes must obey the same name, command and URL
/// allow/deny settings as startup MCP configuration. Denials take priority.
pub fn allows(settings: &Value, server: &McpServerConfig) -> Result<bool> {
    let effective = settings
        .get("effective")
        .context("MCP policy snapshot missing effective settings")?;
    if let Some(denied) = effective.get("deniedMcpServers").filter(|v| !v.is_null()) {
        let entries = denied
            .as_array()
            .context("deniedMcpServers must be an array")?;
        for entry in entries {
            if matches_entry(entry, server)? {
                return Ok(false);
            }
        }
    }
    let managed_only = effective
        .get("allowManagedMcpServersOnly")
        .and_then(Value::as_bool)
        == Some(true);
    let managed = settings
        .get("sources")
        .and_then(Value::as_array)
        .and_then(|sources| {
            sources.iter().find(|source| {
                source.get("source").and_then(Value::as_str) == Some("policySettings")
            })
        })
        .and_then(|source| source.get("settings"));
    let allow_source = if managed_only {
        managed
    } else {
        Some(effective)
    };
    let Some(allowed) = allow_source
        .and_then(|source| source.get("allowedMcpServers"))
        .filter(|v| !v.is_null())
    else {
        return Ok(true);
    };
    let entries = allowed
        .as_array()
        .context("allowedMcpServers must be an array")?;
    if entries.is_empty() {
        return Ok(false);
    }
    let remote = server.transport.as_deref().unwrap_or("stdio") != "stdio";
    let preferred_key = if remote { "serverUrl" } else { "serverCommand" };
    let has_preferred = entries
        .iter()
        .any(|entry| entry.get(preferred_key).is_some());
    for entry in entries {
        if has_preferred && entry.get(preferred_key).is_none() {
            continue;
        }
        if matches_entry(entry, server)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn matches_entry(entry: &Value, server: &McpServerConfig) -> Result<bool> {
    let entry = entry
        .as_object()
        .context("MCP policy entry must be an object")?;
    if let Some(name) = entry.get("serverName") {
        return Ok(name
            .as_str()
            .context("MCP policy serverName must be a string")?
            == server.id);
    }
    if let Some(command) = entry.get("serverCommand") {
        let expected = command
            .as_array()
            .context("MCP policy serverCommand must be an array")?;
        let command = expected
            .iter()
            .map(|item| {
                item.as_str()
                    .context("MCP policy serverCommand entries must be strings")
            })
            .collect::<Result<Vec<_>>>()?;
        return Ok(server.transport.as_deref().unwrap_or("stdio") == "stdio"
            && command.len() == server.args.len() + 1
            && command.first().copied() == Some(server.command.as_str())
            && command
                .iter()
                .skip(1)
                .copied()
                .eq(server.args.iter().map(String::as_str)));
    }
    if let Some(pattern) = entry.get("serverUrl") {
        let pattern = pattern
            .as_str()
            .context("MCP policy serverUrl must be a string")?;
        if server.transport.as_deref().unwrap_or("stdio") == "stdio" {
            return Ok(false);
        }
        let Some(url) = server.url.as_deref() else {
            return Ok(false);
        };
        let pattern = format!("^{}$", regex::escape(pattern).replace(r"\*", ".*"));
        return Ok(regex::Regex::new(&pattern)?.is_match(url));
    }
    bail!("MCP policy entry must specify serverName, serverCommand or serverUrl")
}
