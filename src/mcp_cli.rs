//! Native MCP configuration commands based on the source's
//! commands/mcp/addCommand.ts and services/mcp/utils.ts.
//! User-initiated configuration changes do not execute or connect to servers.
use crate::config::{Config, McpServerConfig};
use anyhow::{bail, Context, Result};
use clap::{Subcommand, ValueEnum};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Scope {
    Local,
    Project,
    User,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Transport {
    Stdio,
    Http,
    Sse,
}

#[derive(Debug, Subcommand)]
pub enum McpCommand {
    /// Add an MCP server, persisting the configuration without connecting to it
    Add {
        #[arg(short = 's', long, value_enum, default_value = "local")]
        scope: Scope,
        #[arg(short = 't', long, value_enum, default_value = "stdio")]
        transport: Transport,
        #[arg(short = 'e', long = "env", value_name = "KEY=VALUE")]
        env: Vec<String>,
        #[arg(short = 'H', long = "header", value_name = "HEADER: VALUE")]
        header: Vec<String>,
        #[arg(long)]
        client_id: Option<String>,
        #[arg(long)]
        callback_port: Option<u16>,
        #[arg(long)]
        client_secret: bool,
        name: String,
        command_or_url: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// List saved user, project and local MCP server configurations
    List {
        /// Only show currently approved and loadable MCP servers
        #[arg(long)]
        active: bool,
        #[arg(long)]
        no_check: bool,
    },
    /// Add MCP server configuration from a JSON object (no network access)
    AddJson {
        name: String,
        json: String,
        #[arg(short = 's', long, value_enum, default_value = "local")]
        scope: Scope,
        #[arg(long)]
        client_secret: bool,
    },
    /// Inspect the saved MCP server configuration without launching a process
    Get {
        name: String,
        #[arg(short = 's', long, value_enum)]
        scope: Option<Scope>,
        #[arg(long)]
        no_check: bool,
    },
    /// Remove MCP project trust decisions; config is left unchanged
    ResetProjectChoices,
    /// Remove an MCP server (require -s if the name exists in multiple scopes)
    Remove {
        name: String,
        #[arg(short = 's', long, value_enum)]
        scope: Option<Scope>,
    },
}

/// Source mcpRemoveHandler/getMcpConfigByName resolve all persisted scopes.
/// We never launch an unapproved project server merely to enumerate it.
fn find_scopes(root: &Path, name: &str) -> Result<Vec<Scope>> {
    let mut scopes = Vec::new();
    if Config::load()?.mcp_servers.iter().any(|x| x.id == name) {
        scopes.push(Scope::User);
    }
    for scope in [Scope::Project, Scope::Local] {
        if scoped_servers(root, scope)?.contains_key(name) {
            scopes.push(scope);
        }
    }
    Ok(scopes)
}

fn scope_name(scope: Scope) -> &'static str {
    match scope {
        Scope::Local => "local",
        Scope::Project => "project",
        Scope::User => "user",
    }
}

fn get_saved_server(root: &Path, name: &str, scope: Scope) -> Result<McpServerConfig> {
    let server = match scope {
        Scope::User => Config::load()?
            .mcp_servers
            .into_iter()
            .find(|s| s.id == name),
        Scope::Project | Scope::Local => scoped_servers(root, scope)?.remove(name),
    };
    server.with_context(|| format!("MCP server {name} not found in {} scope", scope_name(scope)))
}

fn resolve_scope(root: &Path, name: &str, scope: Option<Scope>) -> Result<Scope> {
    if let Some(scope) = scope {
        return Ok(scope);
    }
    let scopes = find_scopes(root, name)?;
    match scopes.as_slice() {
        [] => bail!("MCP server {name} was not found in any scope"),
        [scope] => Ok(*scope),
        _ => bail!(
            "MCP server {name} exists in multiple scopes; specify --scope local, project, or user"
        ),
    }
}

fn parse_server_json(name: &str, input: &str) -> Result<McpServerConfig> {
    if !valid_server_name(name) {
        bail!("MCP server name must contain 1–24 ASCII letters, digits, - or _");
    }
    let mut config: Map<String, Value> = serde_json::from_str::<Value>(input)
        .context("MCP add-json requires valid JSON")?
        .as_object()
        .context("MCP add-json requires an object")?
        .clone();
    // Refuse unknown and unsupported fields rather than losing credentials or
    // creating a misleadingly partial server configuration.
    const ALLOWED: &[&str] = &[
        "type",
        "command",
        "args",
        "env",
        "url",
        "headers",
        "headersHelper",
        "headers_helper",
    ];
    for field in config.keys() {
        if !ALLOWED.contains(&field.as_str()) {
            bail!("Unsupported MCP add-json field: {field}");
        }
    }
    config.insert("id".into(), Value::String(name.to_owned()));
    let server: McpServerConfig = serde_json::from_value(Value::Object(config))
        .context("Invalid MCP add-json server fields")?;
    let mut check = Config::default();
    check.mcp_servers = vec![server.clone()];
    check.validate()?;
    Ok(server)
}

fn valid_server_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 24
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn key_value(strings: &[String], separator: char, label: &str) -> Result<HashMap<String, String>> {
    let mut result = HashMap::new();
    for item in strings {
        let (key, value) = item
            .split_once(separator)
            .with_context(|| format!("Invalid {label}: {item:?}; expected KEY{separator}value"))?;
        if key.trim().is_empty() {
            bail!("Invalid {label}: empty name in {item:?}");
        }
        if separator == '=' {
            // envUtils.ts::parseEnvVars preserves all bytes after the first =.
            result.insert(key.to_owned(), value.to_owned());
        } else {
            // services/mcp/utils.ts::parseHeaders trims names and values.
            result.insert(key.trim().to_owned(), value.trim().to_owned());
        }
    }
    Ok(result)
}

fn saved_path(root: &Path, scope: Scope) -> Result<PathBuf> {
    Ok(match scope {
        Scope::Local => {
            // Never load a trust grant from a version-controlled project file.
            // A cloned repository may contain .atlas/, but this path belongs
            // to the user's private configuration directory.
            let canonical = root.canonicalize().context("Resolve MCP workspace")?;
            let digest = format!(
                "{:x}",
                Sha256::digest(canonical.to_string_lossy().as_bytes())
            );
            let config_path = Config::path()?;
            let config_home = config_path.parent().context("Invalid user config path")?;
            config_home
                .join("mcp-workspaces")
                .join(format!("{digest}.json"))
        }
        Scope::Project => root.join(".mcp.json"),
        Scope::User => Config::path()?,
    })
}

fn read_scoped(path: &Path) -> Result<Map<String, Value>> {
    if !path.exists() {
        return Ok(Map::new());
    }
    if path.symlink_metadata()?.file_type().is_symlink() {
        bail!(
            "MCP configuration cannot be a symbolic link: {}",
            path.display()
        );
    }
    let content =
        fs::read_to_string(path).with_context(|| format!("Read MCP config {}", path.display()))?;
    let doc: Value = serde_json::from_str(crate::json_read::strip_bom(&content))
        .with_context(|| format!("Parse MCP config {}", path.display()))?;
    let servers = doc
        .get("mcpServers")
        .and_then(Value::as_object)
        .context("MCP scoped config requires object field mcpServers")?;
    Ok(servers.clone())
}

fn to_server(name: &str, value: &Value) -> Result<McpServerConfig> {
    let mut data = value
        .as_object()
        .context("MCP server definition must be object")?
        .clone();
    data.insert("id".to_owned(), Value::String(name.to_owned()));
    serde_json::from_value(Value::Object(data)).context("Invalid MCP server definition")
}

fn atomic_save(path: &Path, doc: &Value) -> Result<()> {
    let parent = path.parent().context("MCP config has no directory")?;
    if parent.exists() && parent.symlink_metadata()?.file_type().is_symlink() {
        bail!("MCP configuration directory cannot be a symbolic link")
    }
    fs::create_dir_all(parent)?;
    if path.exists() && path.symlink_metadata()?.file_type().is_symlink() {
        bail!("MCP configuration cannot be a symbolic link")
    }
    let target = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| -> Result<()> {
        let mut handle = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&target)?;
        handle.write_all(serde_json::to_string_pretty(doc)?.as_bytes())?;
        handle.write_all(b"\n")?;
        handle.sync_all()?;
        fs::rename(&target, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(target);
    }
    result
}

/// Project-defined servers can execute local processes. They are only loaded
/// after the current user has explicitly approved them on this machine.
fn project_fingerprint(server: &Value) -> String {
    let bytes = serde_json::to_vec(server).expect("JSON values are always serializable");
    format!("{:x}", Sha256::digest(bytes))
}

/// Bind the user approval to the exact project server definition. A repo
/// checkout cannot replace an approved command, arguments or credentials
/// and retain the permission to launch it.
fn approved_project_servers(root: &Path) -> Result<HashMap<String, String>> {
    let path = saved_path(root, Scope::Local)?;
    if !path.exists() {
        return Ok(HashMap::new());
    }
    if path.symlink_metadata()?.file_type().is_symlink() {
        bail!("MCP local configuration cannot be a symbolic link");
    }
    let value: Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    let mut result = HashMap::new();
    if let Some(approved) = value.get("approvedProjectServers") {
        let approved = approved
            .as_object()
            .context("approvedProjectServers must be an object of hashes")?;
        for (name, hash) in approved {
            result.insert(
                name.clone(),
                hash.as_str()
                    .context("Invalid project MCP approval hash")?
                    .to_owned(),
            );
        }
    }
    Ok(result)
}

fn approve_project(root: &Path, name: &str, server: &Value) -> Result<()> {
    let path = saved_path(root, Scope::Local)?;
    let mut doc = if path.exists() {
        serde_json::from_str::<Value>(&fs::read_to_string(&path)?)?
    } else {
        json!({"mcpServers":{}})
    };
    let object = doc
        .as_object_mut()
        .context("Local MCP config must be an object")?;
    let approved = object.entry("approvedProjectServers").or_insert(json!({}));
    let map = approved
        .as_object_mut()
        .context("approvedProjectServers must be an object")?;
    map.insert(name.to_owned(), json!(project_fingerprint(server)));
    atomic_save(&path, &doc)
}

fn scoped_servers(root: &Path, scope: Scope) -> Result<HashMap<String, McpServerConfig>> {
    let mut result = HashMap::new();
    for (name, value) in read_scoped(&saved_path(root, scope)?)? {
        if !valid_server_name(&name) {
            bail!("Invalid MCP server id: {name}");
        }
        result.insert(name.clone(), to_server(&name, &value)?);
    }
    Ok(result)
}

pub fn active_servers(root: &Path, config: &Config) -> Result<Vec<McpServerConfig>> {
    let mut profiles = config
        .mcp_servers
        .iter()
        .map(|x| (x.id.clone(), x.clone()))
        .collect::<HashMap<_, _>>();
    let approved = approved_project_servers(root)?;
    for (name, server) in scoped_servers(root, Scope::Project)? {
        let fingerprint = project_fingerprint(&serde_json::to_value(&server)?);
        if approved.get(&name) == Some(&fingerprint) {
            profiles.insert(name, server);
        }
    }
    for (name, server) in scoped_servers(root, Scope::Local)? {
        profiles.insert(name, server);
    }
    let mut sorted = profiles.into_values().collect::<Vec<_>>();
    sorted.sort_by(|a, b| a.id.cmp(&b.id));
    let mut candidate = config.clone();
    candidate.mcp_servers = sorted.clone();
    candidate.validate()?;
    Ok(sorted)
}

fn store_server(root: &Path, scope: Scope, server: McpServerConfig) -> Result<PathBuf> {
    let path = saved_path(root, scope)?;
    if matches!(scope, Scope::User) {
        let mut config = Config::load()?;
        if config.mcp_servers.iter().any(|x| x.id == server.id) {
            bail!("MCP server {} already exists in user scope", server.id);
        }
        config.mcp_servers.push(server);
        config.validate()?;
        config.save()?;
    } else {
        let mut servers = read_scoped(&path)?;
        if servers.contains_key(&server.id) {
            bail!("MCP server {} already exists in scope", server.id);
        }
        let name = server.id.clone();
        let mut object = serde_json::to_value(&server)?
            .as_object()
            .cloned()
            .context("MCP invalid serialized server")?;
        object.remove("id");
        let fingerprint_input = Value::Object(object.clone());
        servers.insert(name.clone(), fingerprint_input);
        let mut doc = if path.exists() {
            serde_json::from_str::<Value>(&fs::read_to_string(&path)?)?
        } else {
            json!({})
        };
        doc.as_object_mut()
            .context("MCP config is not JSON object")?
            .insert("mcpServers".into(), Value::Object(servers));
        // Validate saved content before committing, not after starting a process.
        let mut check = Config::default();
        check.mcp_servers = vec![server.clone()];
        check.validate()?;
        atomic_save(&path, &doc)?;
        if matches!(scope, Scope::Project) {
            approve_project(root, &name, &serde_json::to_value(&server)?)?;
        }
    }
    Ok(path)
}

/// Probe a server through the real MCP initialize/discovery handshake.
/// Never start an unapproved project server merely to display status.
async fn health(root: &Path, scope: Scope, server: &McpServerConfig) -> Value {
    if matches!(scope, Scope::Project) {
        let approved = match approved_project_servers(root) {
            Ok(value) => value,
            Err(error) => return json!({"status":"failed","error":format!("{error:#}")}),
        };
        let config = match serde_json::to_value(server) {
            Ok(value) => value,
            Err(error) => return json!({"status":"failed","error":format!("{error:#}")}),
        };
        if approved.get(&server.id) != Some(&project_fingerprint(&config)) {
            return json!({"status":"needs_approval"});
        }
    }
    let response = tokio::time::timeout(
        std::time::Duration::from_secs(12),
        crate::mcp::McpManager::connect_with_root(std::slice::from_ref(server), root),
    ).await;
    match response {
        Ok(Ok(mut manager)) => match manager.statuses() {
            Ok(statuses) => statuses.into_iter().next().unwrap_or_else(|| {
                json!({"status":"failed","error":"Missing MCP server status"})
            }),
            Err(error) => json!({"status":"failed","error":format!("{error:#}")}),
        },
        Ok(Err(error)) => json!({"status":"failed","error":format!("{error:#}")}),
        Err(_) => json!({"status":"failed","error":"MCP health probe timed out"}),
    }
}

pub async fn run(root: &Path, command: &McpCommand) -> Result<String> {
    match command {
        McpCommand::Add {
            scope,
            transport,
            env,
            header,
            client_id,
            callback_port,
            client_secret,
            name,
            command_or_url,
            args,
        } => {
            if !valid_server_name(name) {
                bail!("MCP server name must contain 1–24 ASCII letters, digits, - or _");
            }
            if command_or_url.trim().is_empty() {
                bail!("MCP command or URL cannot be empty");
            }
            if client_id.is_some() || callback_port.is_some() || *client_secret {
                bail!("MCP OAuth options require an interactive credential flow; no credential is saved");
            }
            let mut server = McpServerConfig {
                id: name.clone(),
                command: String::new(),
                transport: None,
                url: None,
                args: vec![],
                env: HashMap::new(),
                headers: HashMap::new(),
                headers_helper: None,
            };
            match transport {
                Transport::Stdio => {
                    if !header.is_empty() {
                        bail!("--header only applies to HTTP MCP servers");
                    }
                    server.command = command_or_url.clone();
                    server.args = args.clone();
                    server.env = key_value(env, '=', "environment variable")?;
                    server.transport = Some("stdio".to_owned());
                    if command_or_url.starts_with("http://")
                        || command_or_url.starts_with("https://")
                    {
                        eprintln!("Warning: URL-like command is configured as stdio; use --transport http for HTTP MCP");
                    }
                }
                Transport::Http => {
                    if !args.is_empty() {
                        bail!("HTTP MCP servers cannot have stdio arguments");
                    }
                    if !env.is_empty() {
                        bail!("--env only applies to stdio MCP servers");
                    }
                    server.transport = Some("http".to_owned());
                    server.url = Some(command_or_url.clone());
                    server.headers = key_value(header, ':', "HTTP header")?;
                }
                Transport::Sse => {
                    bail!("SSE MCP transport is not yet supported by the Rust HTTP connector")
                }
            }
            let path = store_server(root, *scope, server)?;
            Ok(format!("Added MCP server {name} to {}", path.display()))
        }
        McpCommand::AddJson {
            name,
            json,
            scope,
            client_secret,
        } => {
            if *client_secret {
                bail!("Interactive OAuth client secret entry is not yet supported; no configuration was saved");
            }
            let server = parse_server_json(name, json)?;
            let path = store_server(root, *scope, server)?;
            Ok(format!("Added MCP server {name} to {}", path.display()))
        }
        McpCommand::Get { name, scope, no_check } => {
            let scope = resolve_scope(root, name, *scope)?;
            let config = get_saved_server(root, name, scope)?;
            let health = if *no_check { json!({"status":"unchecked"}) }
                else { health(root, scope, &config).await };
            let mut json = serde_json::to_value(config)?;
            json.as_object_mut()
                .context("MCP config serialization is not an object")?
                .insert("scope".into(), Value::String(scope_name(scope).to_owned()));
            json["health"] = health;
            Ok(serde_json::to_string_pretty(&json)?)
        }
        McpCommand::ResetProjectChoices => {
            let path = saved_path(root, Scope::Local)?;
            if !path.exists() {
                return Ok("No approved project MCP servers to reset".to_owned());
            }
            if path.symlink_metadata()?.file_type().is_symlink() {
                bail!("MCP approval configuration cannot be a symbolic link");
            }
            let mut local: Value = serde_json::from_str(&fs::read_to_string(&path)?)?;
            let doc = local
                .as_object_mut()
                .context("Local MCP config must be an object")?;
            doc.remove("approvedProjectServers");
            atomic_save(&path, &local)?;
            Ok("All project MCP approvals have been reset".to_owned())
        }
        McpCommand::List { active, no_check } => {
            if *active {
                let profiles = active_servers(root, &Config::load()?)?;
                let output = profiles
                    .into_iter()
                    .map(|server| {
                        json!({
                            "name":server.id,
                            "transport":server.transport.unwrap_or_else(||"stdio".to_owned()),
                        })
                    })
                    .collect::<Vec<_>>();
                return Ok(serde_json::to_string_pretty(&output)?);
            }
            let mut saved = Vec::new();
            for server in Config::load()?.mcp_servers {
                saved.push((Scope::User, server));
            }
            for scope in [Scope::Project, Scope::Local] {
                for server in scoped_servers(root, scope)?.into_values() {
                    saved.push((scope, server));
                }
            }
            saved.sort_by(|a, b| a.1.id.cmp(&b.1.id));
            let mut output = Vec::with_capacity(saved.len());
            for (scope, server) in saved {
                let mut item = json!({
                    "scope":scope_name(scope), "name":server.id,
                    "transport":server.transport.as_deref().unwrap_or("stdio")
                });
                item["health"] = if *no_check { json!({"status":"unchecked"}) }
                    else { health(root, scope, &server).await };
                output.push(item);
            }
            Ok(serde_json::to_string_pretty(&output)?)
        }
        McpCommand::Remove { name, scope } => {
            let scope = resolve_scope(root, name, *scope)?;
            let path = saved_path(root, scope)?;
            if matches!(scope, Scope::User) {
                let mut config = Config::load()?;
                let previous = config.mcp_servers.len();
                config.mcp_servers.retain(|x| x.id != *name);
                if config.mcp_servers.len() == previous {
                    bail!("MCP server {name} not found in user scope");
                }
                config.save()?;
            } else {
                let mut servers = read_scoped(&path)?;
                if servers.remove(name).is_none() {
                    bail!("MCP server {name} not found");
                }
                let mut doc: Value = serde_json::from_str(&fs::read_to_string(&path)?)?;
                doc.as_object_mut()
                    .context("MCP config is not an object")?
                    .insert("mcpServers".into(), Value::Object(servers));
                atomic_save(&path, &doc)?;
                if matches!(scope, Scope::Project) {
                    let local_path = saved_path(root, Scope::Local)?;
                    if local_path.exists() {
                        let mut local: Value =
                            serde_json::from_str(&fs::read_to_string(&local_path)?)?;
                        if let Some(grants) = local
                            .get_mut("approvedProjectServers")
                            .and_then(Value::as_object_mut)
                        {
                            grants.remove(name);
                            atomic_save(&local_path, &local)?;
                        }
                    }
                }
            }
            Ok(format!("Removed MCP server {name} from {}", path.display()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_source_env_and_header_parsing() {
        assert_eq!(
            key_value(&["API=foo=bar".into()], '=', "env").unwrap()["API"],
            "foo=bar"
        );
        assert_eq!(
            key_value(&["API= spaced value ".into()], '=', "env").unwrap()["API"],
            " spaced value "
        );
        assert_eq!(
            key_value(&[" Authorization : Bearer secret ".into()], ':', "header").unwrap()
                ["Authorization"],
            "Bearer secret"
        );
        assert!(key_value(&["NO_EQUAL".into()], '=', "env").is_err());
        assert!(key_value(&[" :test".into()], ':', "header").is_err());
        assert!(valid_server_name("valid-name_8"));
        assert!(!valid_server_name("server/../../bad"));
    }

    #[test]
    fn approvals_are_pinned_to_exact_project_configuration() {
        let a = json!({"type":"stdio","command":"python","args":["-m","server"],"env":{"X":"1"}});
        let b = json!({"type":"stdio","command":"sh","args":["-m","server"],"env":{"X":"1"}});
        assert_ne!(project_fingerprint(&a), project_fingerprint(&b));
        assert_eq!(project_fingerprint(&a), project_fingerprint(&a));
    }
}
