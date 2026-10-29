use crate::{config::McpServerConfig, hooks::{HookEvent, HookRule}};
use serde_json::Value;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Deserialize)]
struct Manifest {
    name: String,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    hooks: Value,
    #[serde(default, alias = "mcpServers")]
    mcp_servers: Value,
    #[serde(default)]
    skills: Value,
    #[serde(default)]
    commands: Value,
    #[serde(default, alias = "userConfig")]
    user_config: Value,
}

fn checked_component(root: &Path, path: &Path) -> Result<PathBuf> {
    let canonical = path.canonicalize()
        .with_context(|| format!("Cannot read plugin component {}", path.display()))?;
    if !canonical.starts_with(root) {
        bail!("Plugin component leaves plugin root: {}", path.display())
    }
    Ok(canonical)
}

/// Original pluginLoader.ts::loadPluginManifest allows a plugin with only
/// commands, agents or skills, and reads .claude-plugin/plugin.json.
fn read_manifest(root: &Path, fallback: &str) -> Result<Manifest> {
    let standard = root.join(".claude-plugin/plugin.json");
    let legacy = root.join("plugin.json");
    let path = if standard.exists() { Some(standard) }
        else if legacy.exists() { Some(legacy) } else { None };
    let Some(path) = path else {
        return Ok(Manifest {
            name: fallback.to_owned(), version: None,
            hooks: Value::Null, mcp_servers: Value::Null,
            skills: Value::Null, commands: Value::Null,
            user_config: Value::Null,
        });
    };
    let path = checked_component(root, &path)?;
    let manifest: Manifest = serde_json::from_slice(&fs::read(&path)?)
        .with_context(|| format!("Invalid plugin manifest {}", path.display()))?;
    if !valid_name(&manifest.name) {
        bail!("Invalid plugin name in manifest")
    }
    Ok(manifest)
}

fn source_hooks(root: &Path, value: &Value) -> Result<Vec<HookRule>> {
    let mut rules = Vec::new();
    let events = value.as_object().context("Plugin hooks must be an event map")?;
    for (event_name, matchers) in events {
        let event: HookEvent = serde_json::from_value(Value::String(event_name.clone()))
            .with_context(|| format!("Unsupported plugin hook event {event_name}"))?;
        for matcher in matchers.as_array().context("Plugin hook matchers must be an array")? {
            let matcher_name = matcher.get("matcher").and_then(Value::as_str).map(str::to_owned);
            let entries = matcher.get("hooks").and_then(Value::as_array)
                .context("Plugin hook matcher requires hooks array")?;
            for hook in entries {
                if hook.get("type").and_then(Value::as_str) != Some("command") {
                    bail!("Plugin hook type requires a command handler")
                }
                let command = hook.get("command").and_then(Value::as_str)
                    .context("Plugin command hook requires command")?;
                let timeout_seconds = match hook.get("timeout") {
                    Some(v) => {
                        let seconds = v.as_u64()
                            .context("Plugin hook timeout must be a nonnegative integer")?;
                        if seconds == 0 { 600 } else { seconds }
                    },
                    None => 600,
                };
                if hook.get("async").and_then(Value::as_bool) == Some(true) {
                    bail!("Asynchronous plugin command hooks are not yet supported")
                }
                if hook.get("once").and_then(Value::as_bool) == Some(true) {
                    bail!("One-shot plugin command hooks require lifecycle tracking")
                }
                let shell = hook.get("shell").map(|value|
                    serde_json::from_value(value.clone())).transpose()?;
                rules.push(HookRule {
                    event: event.clone(), matcher: matcher_name.clone(),
                    if_condition: matcher.get("if").and_then(Value::as_str).map(str::to_owned),
                    plugin_root: Some(root.to_owned()),
                    command: command.to_owned(), shell, timeout_seconds,
                });
            }
        }
    }
    Ok(rules)
}

fn hook_file(root: &Path, path: &Path) -> Result<Vec<HookRule>> {
    let path = checked_component(root, path)?;
    let document: Value = serde_json::from_slice(&fs::read(&path)?)
        .with_context(|| format!("Invalid plugin hooks file {}", path.display()))?;
    let hooks = document.get("hooks").context("Plugin hooks file requires hooks object")?;
    source_hooks(root, hooks)
}

fn manifest_hooks(root: &Path, value: &Value) -> Result<Vec<HookRule>> {
    let mut hooks = Vec::new();
    let mut loaded = std::collections::HashSet::new();
    let standard = root.join("hooks/hooks.json");
    if standard.exists() {
        let real = checked_component(root, &standard)?;
        loaded.insert(real.clone());
        hooks.extend(hook_file(root, &real)?);
    }
    let specifications: Vec<&Value> = match value {
        Value::Null => Vec::new(),
        Value::Array(items) => items.iter().collect(),
        other => vec![other],
    };
    for spec in specifications {
        if let Some(relative) = spec.as_str() {
            let path = checked_component(root, &root.join(relative))?;
            if !loaded.insert(path.clone()) {
                continue;
            }
            hooks.extend(hook_file(root, &path)?);
        } else if spec.get("event").is_some() && spec.get("command").is_some() {
            let mut hook: HookRule = serde_json::from_value(spec.clone())?;
            hook.plugin_root = Some(root.to_owned());
            hooks.push(hook);
        } else if spec.is_object() {
            hooks.extend(source_hooks(root, spec)?);
        } else {
            bail!("Invalid plugin hook specification")
        }
    }
    Ok(hooks)
}

/// mcpPluginIntegration.ts loads .mcp.json first, then merges the manifest's
/// mcpServers (inline map, file path or ordered array) with later entries
/// overriding earlier entries for the same server name.
fn mcp_file(root: &Path, relative: &str) -> Result<Option<Value>> {
    let requested = root.join(relative);
    // Optional plugin MCP files are ignored when absent, as in the source
    // loader. Existing paths must still pass the plugin-root boundary check.
    if !requested.exists() && !requested.is_symlink() {
        return Ok(None);
    }
    let path = checked_component(root, &requested)?;
    let document: Value = match fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok()) {
        Some(document) => document,
        None => return Ok(None),
    };
    Ok(Some(document.get("mcpServers").cloned().unwrap_or(document)))
}

fn merge_server_specs(
    root: &Path,
    value: &Value,
    servers: &mut std::collections::BTreeMap<String, Value>,
    visiting: &mut std::collections::HashSet<PathBuf>,
) -> Result<()> {
    match value {
        Value::Null => {}
        Value::String(relative) => {
            if relative.ends_with(".mcpb") {
                bail!("MCPB plugin bundles require an installed bundle handler")
            }
            let requested = root.join(relative);
            if !requested.exists() && !requested.is_symlink() {
                return Ok(());
            }
            let file = checked_component(root, &requested)?;
            if !visiting.insert(file.clone()) {
                bail!("Plugin MCP configuration contains a file reference cycle")
            }
            let result = match mcp_file(root, relative)? {
                Some(document) => merge_server_specs(root, &document, servers, visiting),
                None => Ok(()),
            };
            visiting.remove(&file);
            result?;
        }
        Value::Array(specs) => {
            // The source loader resolves each manifest entry independently,
            // then merges successful entries in declaration order. An invalid
            // entry must neither discard earlier servers nor leave a partial
            // merge that changes the meaning of later overrides.
            for spec in specs {
                let mut candidate = servers.clone();
                let mut candidate_visiting = visiting.clone();
                let result = if let Some(id) = spec.get("id").and_then(Value::as_str) {
                    candidate.insert(id.to_owned(), spec.clone());
                    Ok(())
                } else {
                    merge_server_specs(root, spec, &mut candidate, &mut candidate_visiting)
                };
                if result.is_ok() {
                    *servers = candidate;
                }
            }
        }
        Value::Object(entries) => {
            for (name, config) in entries {
                servers.insert(name.clone(), config.clone());
            }
        }
        _ => bail!("Plugin MCP servers must be a map, path or array"),
    }
    Ok(())
}

fn manifest_servers(root: &Path, value: &Value) -> Result<Vec<McpServerConfig>> {
    let mut entries = std::collections::BTreeMap::new();
    let mut visiting = std::collections::HashSet::new();
    let standard = root.join(".mcp.json");
    if standard.exists() {
        if let Some(document) = mcp_file(root, ".mcp.json")? {
            merge_server_specs(root, &document, &mut entries, &mut visiting)?;
        }
    }
    merge_server_specs(root, value, &mut entries, &mut visiting)?;
    let mut servers = Vec::new();
    for (name, config) in entries {
        let mut config = config;
        let Some(object) = config.as_object_mut() else {
            continue;
        };
        object.insert("id".into(), Value::String(name.clone()));
        // A malformed server entry must not disable healthy servers in the
        // same plugin. The original loader validates entries independently.
        if let Ok(server) = serde_json::from_value::<McpServerConfig>(config) {
            // McpServerConfigSchema in the source rejects empty commands
            // and missing remote URLs before attempting any connection.
            // Serde defaults alone do not enforce these requirements.
            let valid = match server.transport.as_deref().unwrap_or("stdio") {
                "stdio" => !server.command.is_empty(),
                "http" | "sse" | "ws" => server.url.as_deref()
                    .is_some_and(|url| !url.is_empty()),
                _ => true,
            };
            if valid {
                servers.push(server);
            }
        }
    }
    Ok(servers)
}

/// Resolve plugin-scoped variables before handing a server to the shared MCP
/// transport. Plugin paths are stable across subprocess startup and must not
/// depend on the caller's ambient environment.
fn resolve_plugin_mcp_server(
    root: &Path,
    plugin_name: &str,
    manifest: &Manifest,
    mut server: McpServerConfig,
) -> Result<McpServerConfig> {
    let plugin_root = if cfg!(windows) {
        root.to_string_lossy().replace('\\', "/")
    } else {
        root.to_string_lossy().into_owned()
    };
    let storage_root = root.parent().and_then(Path::parent)
        .context("Plugin installation has no storage directory")?;
    let data = storage_root.join("plugin-data").join(plugin_name);
    // Reject redirected storage: plugins must not turn an environment
    // expansion into a write outside their installation's workspace.
    if data.is_symlink() || data.parent().is_some_and(|parent| parent.is_symlink()) {
        bail!("Plugin data directory is redirected");
    }
    if server.transport.as_deref().unwrap_or("stdio") == "stdio" {
        fs::create_dir_all(&data)?;
    }
    let plugin_data = if cfg!(windows) {
        data.to_string_lossy().replace('\\', "/")
    } else {
        data.to_string_lossy().into_owned()
    };
    // mcpPluginIntegration.ts::buildMcpUserConfig/resolvePluginMcpEnvironment:
    // expand saved user_config values in the actual MCP launch arguments,
    // headers and URLs before resolving ordinary environment variables.
    // Secrets remain outside public settings until secure storage is ported.
    let config_schema = manifest.user_config.as_object();
    let user_config_pattern = regex::Regex::new(r"\$\{user_config\.([^}]+)\}")?;
    let needs_user_config = std::iter::once(server.command.as_str())
        .chain(server.args.iter().map(String::as_str))
        .chain(server.env.values().map(String::as_str))
        .chain(server.url.iter().map(String::as_str))
        .chain(server.headers.values().map(String::as_str))
        .any(|value| user_config_pattern.is_match(value));
    let options = if needs_user_config && config_schema.is_some() {
        let workspace = root.parent().and_then(Path::parent).and_then(Path::parent)
            .context("Plugin installation is outside the workspace")?;
        Some(crate::permission_settings::SettingsPaths::for_workspace(workspace)?
            .plugin_public_options(plugin_name)?)
    } else {
        None
    };
    let expand = |raw: &str| -> Result<String> {
        let mut substituted = String::with_capacity(raw.len());
        let mut last = 0;
        for capture in user_config_pattern.captures_iter(raw) {
            let whole = capture.get(0).context("Invalid user config reference")?;
            substituted.push_str(&raw[last..whole.start()]);
            let key = &capture[1];
            if let Some(options) = &options {
                let field = config_schema.and_then(|schema| schema.get(key))
                    .with_context(|| format!("Undeclared plugin user configuration: {key}"))?;
                if field.get("sensitive").and_then(Value::as_bool) == Some(true) {
                    bail!("Plugin user configuration '{key}' requires secure secret storage")
                }
                let value = options.get(key)
                    .with_context(|| format!("Missing required plugin user configuration: {key}"))?;
                // mcpbHandler.ts::validateUserConfig: reject wrong types before
                // launching a process with an unusable command or header.
                let kind = field.get("type").and_then(Value::as_str);
                let multiple = field.get("multiple").and_then(Value::as_bool) == Some(true);
                let valid = match kind {
                    Some("string") if multiple => value.is_string()
                        || value.as_array().is_some_and(|items| items.iter().all(Value::is_string)),
                    Some("string" | "file" | "directory") => value.is_string(),
                    Some("number") => value.is_number(),
                    Some("boolean") => value.is_boolean(),
                    _ => true,
                };
                if !valid {
                    bail!("Invalid plugin user configuration type for '{key}'")
                }
                if let Some(number) = value.as_f64() {
                    if field.get("min").and_then(Value::as_f64).is_some_and(|min| number < min)
                        || field.get("max").and_then(Value::as_f64).is_some_and(|max| number > max) {
                        bail!("Plugin user configuration '{key}' is outside its allowed range")
                    }
                }
                match value {
                    Value::String(value) => substituted.push_str(value),
                    Value::Array(items) => substituted.push_str(&items.iter()
                        .map(|item| item.as_str().map(str::to_owned)
                            .unwrap_or_else(|| item.to_string()))
                        .collect::<Vec<_>>().join(",")),
                    other => substituted.push_str(&other.to_string()),
                }
            } else {
                substituted.push_str(whole.as_str());
            }
            last = whole.end();
        }
        substituted.push_str(&raw[last..]);
        // Keep unresolved ambient variables intact: the original plugin
        // loader records them and reports a per-server connection failure.
        Ok(crate::compat_helpers::expand_env_vars_with(&substituted, |key| {
            match key {
                "ATLAS_PLUGIN_ROOT" | "CLAUDE_PLUGIN_ROOT" => Some(plugin_root.clone()),
                "ATLAS_PLUGIN_DATA" | "CLAUDE_PLUGIN_DATA" => Some(plugin_data.clone()),
                _ => std::env::var(key).ok(),
            }
        }).expanded)
    };
    match server.transport.as_deref().unwrap_or("stdio") {
        "stdio" => {
            server.command = expand(&server.command)?;
            server.args = server.args.iter().map(|arg| expand(arg))
                .collect::<Result<Vec<_>>>()?;
            server.env = server.env.iter()
                .map(|(key, value)| Ok((key.clone(), expand(value)?)))
                .collect::<Result<_>>()?;
            server.env.insert("ATLAS_PLUGIN_ROOT".into(), plugin_root);
            server.env.insert("ATLAS_PLUGIN_DATA".into(), plugin_data);
        }
        "http" | "sse" | "ws" => {
            if let Some(url) = &server.url {
                server.url = Some(expand(url)?);
            }
            server.headers = server.headers.iter()
                .map(|(key, value)| Ok((key.clone(), expand(value)?)))
                .collect::<Result<_>>()?;
        }
        _ => {}
    }
    Ok(server)
}

/// Validate an unpacked plugin before installing it. The original loader
/// accepts a directory name when .claude-plugin/plugin.json is absent.
pub fn validate_local(location: &Path) -> Result<String> {
    let location = location.canonicalize().context("Plugin directory unavailable")?;
    if !location.is_dir() {
        bail!("Plugin source must be a directory")
    }
    let fallback = location.file_name().and_then(|name| name.to_str())
        .context("Plugin source has no valid directory name")?;
    if !valid_name(fallback) {
        bail!("Invalid plugin directory name")
    }
    let manifest = read_manifest(&location, fallback)?;
    let _ = manifest_hooks(&location, &manifest.hooks)?;
    let _ = manifest_servers(&location, &manifest.mcp_servers)?;
    let _ = skill_roots(&location, &manifest)?;
    Ok(manifest.name)
}

pub struct PluginSet {
    pub hooks: Vec<HookRule>,
    pub mcp_servers: Vec<McpServerConfig>,
    /// Failed plugin MCP configurations, kept visible to SDK reload callers.
    pub errors: Vec<String>,
}
fn valid_name(value: &str) -> bool {
    // Keep plugin discovery aligned with installation and the source
    // marketplace identifier schema; long and dotted names are valid.
    let mut bytes = value.bytes();
    matches!(bytes.next(), Some(c) if c.is_ascii_alphanumeric())
        && bytes.all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.'))
}
/// Preserve marketplace identity for managed policy checks even after a
/// plugin has been installed. Only the installer may create the origin file.
pub(crate) fn plugin_blocked_by_policy(root: &Path, name: &str) -> Result<bool> {
    let settings = crate::permission_settings::SettingsPaths::for_workspace(root)?;
    if settings.plugin_blocked_by_policy(name)? {
        return Ok(true);
    }
    let installed = root.join(".atlas/plugins").join(name);
    let metadata = match fs::symlink_metadata(&installed) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    if metadata.file_type().is_symlink() {
        bail!("Plugin installation cannot be a symbolic link");
    }
    let origin = installed.join(".atlas-plugin-origin.json");
    let metadata = match fs::symlink_metadata(&origin) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        bail!("Plugin marketplace identity is not a regular file");
    }
    let value: Value = serde_json::from_slice(&fs::read(origin)?)?;
    let plugin_id = value.get("pluginId").and_then(Value::as_str)
        .context("Installed plugin marketplace identity is invalid")?;
    settings.plugin_blocked_by_policy(plugin_id)
}
fn directory(root: &Path, name: &str) -> Result<PathBuf> {
    if !valid_name(name) {
        bail!("Invalid plugin id: {name}")
    }
    if plugin_blocked_by_policy(root, name)? {
        bail!("Plugin '{name}' is disabled by managed policy");
    }
    let base = root.join(".atlas/plugins").join(name);
    let real = base
        .canonicalize()
        .with_context(|| format!("Plugin '{name}' is not installed"))?;
    if !real.starts_with(root) {
        bail!("Plugin '{name}' leaves workspace")
    }
    if !real.is_dir() {
        bail!("Plugin '{name}' is not a directory")
    }
    Ok(real)
}
impl PluginSet {
    pub fn load(root: &Path, enabled: &[String]) -> Result<Self> {
        let mut set = Self {
            hooks: Vec::new(),
            mcp_servers: Vec::new(),
            errors: Vec::new(),
        };
        let mut seen = std::collections::HashSet::new();
        for name in enabled {
            if !seen.insert(name) {
                bail!("Plugin {name} enabled twice")
            }
            if plugin_blocked_by_policy(root, name)? {
                set.errors.push(format!("Plugin '{name}' disabled by managed policy"));
                continue;
            }
            let location = directory(root, name)?;
            let manifest = read_manifest(&location, name)?;
            if manifest.name != *name {
                bail!("Plugin name does not match directory: {name}")
            }
            // Version is metadata; no external update or installation happens.
            let _version = &manifest.version;
            // Keep the validated canonical plugin directory attached to each
            // hook. Two plugins using the same template command must both
            // execute, and substitution must resolve inside the right plugin.
            for mut hook in manifest_hooks(&location, &manifest.hooks)? {
                hook.plugin_root = Some(location.clone());
                set.hooks.push(hook);
            }
            for server in manifest_servers(&location, &manifest.mcp_servers)? {
                // A single unconfigured MCP server must not prevent other
                // tools, skills and hooks in this plugin from loading.
                let server_id = server.id.clone();
                let mut server = match resolve_plugin_mcp_server(&location, name, &manifest, server) {
                    Ok(server) => server,
                    Err(error) => {
                        let diagnostic = format!("Plugin '{name}' MCP server '{server_id}' unavailable: {error:#}");
                        eprintln!("{diagnostic}");
                        set.errors.push(diagnostic);
                        continue;
                    }
                };
                // Plugin-scoped MCP names may exceed the local config's
                // short identifier limit; the original source scopes the full
                // plugin and server names without truncating or rejecting them.
                server.id = format!("{}_{}", name, server.id);
                set.mcp_servers.push(server);
            }
        }
        Ok(set)
    }
}
/// pluginLoader.ts::createPluginFromPath accepts manifest.skills as a
/// relative directory (or array of directories), overriding the default
/// skills/ directory. Keep every resolved directory inside this plugin.
fn skill_roots(plugin_root: &Path, manifest: &Manifest) -> Result<Vec<PathBuf>> {
    let paths: Vec<String> = match &manifest.skills {
        Value::Null => vec!["skills".into()],
        Value::String(path) => vec![path.clone()],
        Value::Array(paths) => paths.iter().map(|path| path.as_str()
            .map(str::to_owned).context("Plugin skill path must be a string"))
            .collect::<Result<Vec<_>>>()?,
        _ => bail!("Plugin skills must be a relative path or array of paths"),
    };
    let mut roots = Vec::new();
    for path in paths {
        let target = plugin_root.join(&path);
        if !target.exists() {
            if manifest.skills.is_null() {
                continue;
            }
            bail!("Plugin skills directory does not exist: {path}")
        }
        let target = checked_component(plugin_root, &target)?;
        if !target.is_dir() {
            bail!("Plugin skills path must be a directory: {path}")
        }
        if !roots.contains(&target) {
            roots.push(target);
        }
    }
    Ok(roots)
}

fn skill_files(plugin_root: &Path, manifest: &Manifest) -> Result<Vec<(String, PathBuf)>> {
    let mut files = std::collections::BTreeMap::new();
    for root in skill_roots(plugin_root, manifest)? {
        // loadPluginCommands.ts::getCommandNameFromFile uses colon-delimited
        // namespaces for nested SKILL.md directories. Do not follow links.
        for entry in walkdir::WalkDir::new(&root).follow_links(false) {
            let entry = entry?;
            if !entry.file_type().is_file()
                || !entry.file_name().to_string_lossy().eq_ignore_ascii_case("skill.md") {
                continue;
            }
            let parent = entry.path().parent().context("Invalid plugin skill path")?;
            let relative = parent.strip_prefix(&root)?;
            let mut parts = Vec::new();
            for component in relative.components() {
                let part = component.as_os_str().to_str().context("Invalid skill namespace")?;
                if !valid_name(part) {
                    bail!("Invalid plugin skill namespace: {part}")
                }
                parts.push(part.to_owned());
            }
            if parts.is_empty() {
                let name = root.file_name().and_then(|name| name.to_str())
                    .context("Invalid skill directory name")?;
                if !valid_name(name) {
                    bail!("Invalid plugin skill name: {name}")
                }
                parts.push(name.to_owned());
            }
            let name = parts.join(":");
            let file = checked_component(plugin_root, entry.path())?;
            files.insert(name, file);
        }
    }
    Ok(files.into_iter().collect())
}

pub fn list_skills(root: &Path, enabled: &[String]) -> Result<Vec<String>> {
    let mut names = Vec::new();
    for name in enabled {
        if plugin_blocked_by_policy(root, name)? {
            continue;
        }
        let plugin_root = directory(root, name)?;
        let manifest = read_manifest(&plugin_root, name)?;
        for (skill, _) in skill_files(&plugin_root, &manifest)? {
            names.push(format!("{name}:{skill}"));
        }
    }
    names.sort();
    Ok(names)
}
/// Resolve the same manifest-configured and nested skill file used by
/// discovery. The tool executor must check permissions against this real
/// canonical path, not assume every skill lives under skills/<name>/SKILL.md.
pub fn resolve_skill_path(root: &Path, enabled: &[String], identifier: &str) -> Result<PathBuf> {
    let (name, skill) = identifier
        .split_once(':')
        .context("Use plugin:skill as plugin skill name")?;
    if !valid_name(name) || !skill.split(':').all(valid_name)
        || !enabled.iter().any(|n| n == name) {
        bail!("Plugin skill not enabled")
    }
    // Discovery filters managed-disabled plugins, but a direct Skill tool
    // call can still supply an old identifier. Recheck policy at use time.
    if plugin_blocked_by_policy(root, name)? {
        bail!("Plugin '{name}' disabled by managed policy")
    }
    let plugin_root = directory(root, name)?;
    let manifest = read_manifest(&plugin_root, name)?;
    skill_files(&plugin_root, &manifest)?
        .into_iter().find(|(candidate, _)| candidate == skill)
        .map(|(_, path)| path).context("Plugin skill not found")
}

pub fn read_skill(root: &Path, enabled: &[String], identifier: &str) -> Result<String> {
    let path = resolve_skill_path(root, enabled, identifier)?;
    Ok(fs::read_to_string(path)?)
}

/// The original plugin command loader keeps user-invocable: false commands
/// available to the model but excludes them from direct slash invocation.
/// A frontmatter key only applies at the top level of a closed YAML header.
fn user_invocable_command(content: &str) -> bool {
    crate::skill_arguments::frontmatter_boolean(content, "user-invocable").unwrap_or(true)
}

/// Provider-neutral, per-plugin persistent data storage. The source lazily
/// creates a plugin data directory only when its prompt references the token.
fn plugin_data_directory(plugin_root: &Path) -> Result<String> {
    let name = plugin_root.file_name().and_then(|part| part.to_str())
        .context("Invalid plugin directory name")?;
    if !valid_name(name) {
        bail!("Invalid plugin directory name")
    }
    let atlas = plugin_root.parent().and_then(Path::parent)
        .context("Plugin installation is outside the configured plugin tree")?;
    let base = atlas.join("plugin-data");
    fs::create_dir_all(&base)?;
    let base = base.canonicalize()?;
    if !base.starts_with(atlas) {
        bail!("Plugin data directory leaves the workspace")
    }
    let data = base.join(name);
    fs::create_dir_all(&data)?;
    let data = data.canonicalize()?;
    if !data.starts_with(&base) {
        bail!("Plugin data directory leaves the plugin data root")
    }
    Ok(data.to_string_lossy().replace('\\', "/"))
}

/// Plugin option values in user settings are non-sensitive; the original
/// stores secrets separately. Never place a sensitive value in skill prose,
/// even when a user mistakenly put it in the public settings file.
fn substitute_plugin_user_config(prompt: &str, manifest: &Manifest, plugin_root: &Path) -> Result<String> {
    let Some(schema) = manifest.user_config.as_object() else {
        if manifest.user_config.is_null() { return Ok(prompt.to_owned()); }
        bail!("Plugin userConfig schema must be an object")
    };
    if !prompt.contains("${user_config.") {
        return Ok(prompt.to_owned());
    }
    let workspace = plugin_root.parent().and_then(Path::parent).and_then(Path::parent)
        .context("Plugin root is outside the local workspace plugin tree")?;
    let options = crate::permission_settings::SettingsPaths::for_workspace(workspace)?
        .plugin_public_options(&manifest.name)?;
    let reference = regex::Regex::new(r"\$\{user_config\.([^}]+)\}")?;
    Ok(reference.replace_all(prompt, |captures: &regex::Captures<'_>| {
        let key = &captures[1];
        if schema.get(key).and_then(|value| value.get("sensitive"))
            .and_then(Value::as_bool) == Some(true) {
            return format!("[sensitive option '{key}' not available in skill content]");
        }
        match options.get(key) {
            Some(Value::String(value)) => value.clone(),
            Some(Value::Null) | None => captures[0].to_owned(),
            Some(value) => value.to_string(),
        }
    }).into_owned())
}

fn command_with_user_config(
    prompt: Option<String>, manifest: &Manifest, plugin_root: &Path,
    session_id: Option<&str>,
) -> Result<Option<String>> {
    if session_id.is_none() {
        return Ok(prompt);
    }
    prompt.map(|text| substitute_plugin_user_config(&text, manifest, plugin_root)).transpose()
}

/// Apply the same user-config safety rules when the model reads a plugin
/// skill directly, rather than through the user's slash command.
pub fn expand_skill_user_config(
    root: &Path, enabled: &[String], identifier: &str, prompt: &str,
) -> Result<String> {
    let (plugin, _) = identifier.split_once(':')
        .context("Plugin skill requires plugin:name")?;
    if !valid_name(plugin) || !enabled.iter().any(|name| name == plugin) {
        bail!("Plugin skill not enabled")
    }
    // Model-side skill rendering can bypass command discovery; a policy
    // change must invalidate the direct rendering path as well.
    if plugin_blocked_by_policy(root, plugin)? {
        bail!("Plugin '{plugin}' disabled by managed policy")
    }
    let location = directory(root, plugin)?;
    let manifest = read_manifest(&location, plugin)?;
    substitute_plugin_user_config(prompt, &manifest, &location)
}

/// Expand the plugin root after argument substitution, as the original
/// getPromptForCommand does. Never treat the expanded text as a shell command.
fn plugin_command_body(content: &str, args: &str, root: &Path, session_id: Option<&str>) -> Result<Option<String>> {
    if !user_invocable_command(content) {
        return Ok(None);
    }
    let names = crate::skill_arguments::frontmatter_argument_names(content);
    let prompt = crate::skill_arguments::substitute_arguments(
        crate::skill_arguments::markdown_body(content), Some(args), true, &names,
    );
    let root_text = root.to_string_lossy();
    #[cfg(windows)]
    let root_text = root_text.replace('\\', "/");
    let prompt = prompt.replace("${CLAUDE_PLUGIN_ROOT}", root_text.as_ref());
    let prompt = match session_id {
        Some(id) => prompt.replace("${CLAUDE_SESSION_ID}", id),
        None => prompt,
    };
    let prompt = if session_id.is_some() && prompt.contains("${CLAUDE_PLUGIN_DATA}") {
        prompt.replace("${CLAUDE_PLUGIN_DATA}", &plugin_data_directory(root)?)
    } else {
        prompt
    };
    Ok(Some(prompt))
}

/// Resolve an original plugin Markdown slash command to its model-visible
/// prompt. The interactive entrypoint calls this before the ordinary user
/// prompt path, so a plugin command actually executes instead of being sent
/// as unrecognized literal text.
pub fn command_prompt(
    root: &Path, enabled: &[String], command: &str, args: &str,
) -> Result<Option<String>> {
    command_prompt_with_session(root, enabled, command, args, None)
}

/// Interactive invocation additionally exposes the current session identifier,
/// as the original getPromptForCommand substitutes getSessionId() at call time.
pub fn command_prompt_with_session(
    root: &Path, enabled: &[String], command: &str, args: &str,
    session_id: Option<&str>,
) -> Result<Option<String>> {
    let Some((plugin, suffix)) = command.split_once(':') else {
        return Ok(None);
    };
    if !enabled.iter().any(|name| name == plugin)
        || !valid_name(plugin) || !suffix.split(':').all(valid_name) {
        return Ok(None);
    }
    // A previously advertised /plugin:command remains addressable even
    // after policy disables it. Reject execution before loading its body.
    if plugin_blocked_by_policy(root, plugin)? {
        bail!("Plugin '{plugin}' disabled by managed policy")
    }
    let plugin_root = directory(root, plugin)?;
    let manifest = read_manifest(&plugin_root, plugin)?;
    if manifest.name != plugin {
        bail!("Plugin manifest identity does not match installed name")
    }

    // Plugin skills are slash commands in the original loader as well as
    // model-visible skills. Resolve the same manifest paths as read_skill.
    if let Some((_, path)) = skill_files(&plugin_root, &manifest)?
        .into_iter().find(|(name, _)| name == suffix) {
        let content = fs::read_to_string(&path)?;
        let prompt = plugin_command_body(&content, args, &plugin_root, session_id)?;
        if prompt.is_none() && session_id.is_some() {
            bail!("Plugin skill is not user invocable")
        }
        let Some(prompt) = prompt else {
            return Ok(None);
        };
        let dir = path.parent().context("Plugin skill has no parent directory")?
            .to_string_lossy().replace('\\', "/");
        let prompt = command_with_user_config(
            Some(prompt), &manifest, &plugin_root, session_id,
        )?.context("Plugin skill command disappeared")?;
        return Ok(Some(format!("Base directory for this skill: {dir}\n\n{prompt}")
            .replace("${CLAUDE_SKILL_DIR}", &dir)));
    }

    // Manifest commands may be inline content or a source file, or an
    // alternate directory list. These are distinct source formats.
    if let Some(entries) = manifest.commands.as_object() {
        if let Some(spec) = entries.get(suffix) {
            let content = if let Some(content) = spec.get("content").and_then(Value::as_str) {
                content.to_owned()
            } else if let Some(relative) = spec.get("source").and_then(Value::as_str) {
                let path = checked_component(&plugin_root, &plugin_root.join(relative))?;
                fs::read_to_string(path)?
            } else {
                bail!("Plugin command requires source or content")
            };
            let prompt = plugin_command_body(&content, args, &plugin_root, session_id)?;
            if prompt.is_none() && session_id.is_some() {
                bail!("Plugin command is not user invocable")
            }
            return command_with_user_config(prompt, &manifest, &plugin_root, session_id);
        }
        return Ok(None);
    }

    let paths: Vec<String> = match &manifest.commands {
        Value::Null => vec!["commands".into()],
        Value::String(path) => vec![path.clone()],
        Value::Array(paths) => paths.iter().map(|path| path.as_str()
            .map(str::to_owned).context("Plugin command path must be a string"))
            .collect::<Result<Vec<_>>>()?,
        _ => bail!("Plugin commands must be a path, array or metadata map"),
    };
    for relative in paths {
        let candidate = plugin_root.join(&relative);
        if !candidate.exists() {
            if manifest.commands.is_null() {
                continue;
            }
            bail!("Plugin command path does not exist: {relative}")
        }
        let candidate = checked_component(&plugin_root, &candidate)?;
        let mut markdown = Vec::new();
        if candidate.is_file() {
            markdown.push(candidate.clone());
        } else if candidate.is_dir() {
            for entry in walkdir::WalkDir::new(&candidate).follow_links(false) {
                let entry = entry?;
                if entry.file_type().is_file()
                    && entry.path().extension().and_then(|ext| ext.to_str())
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("md")) {
                    markdown.push(checked_component(&plugin_root, entry.path())?);
                }
            }
        } else {
            bail!("Plugin command source is not a file or directory")
        }
        for path in markdown {
            let relative_path = path.strip_prefix(&candidate).unwrap_or(path.as_path());
            let is_skill = path.file_name().and_then(|name| name.to_str())
                .is_some_and(|name| name.eq_ignore_ascii_case("SKILL.md"));
            if !is_skill && path.parent().is_some_and(|parent| parent.join("SKILL.md").is_file()) {
                continue;
            }
            let mut components = Vec::new();
            if let Some(parent) = relative_path.parent() {
                for component in parent.components() {
                    let part = component.as_os_str().to_str().context("Invalid plugin command path")?;
                    if !part.is_empty() {
                        components.push(part.to_owned());
                    }
                }
            }
            if !is_skill {
                let stem = path.file_stem().and_then(|stem| stem.to_str())
                    .context("Invalid plugin command filename")?;
                components.push(stem.to_owned());
            } else if components.is_empty() {
                let parent = path.parent().and_then(|parent| parent.file_name())
                    .and_then(|name| name.to_str()).context("Invalid skill command directory")?;
                components.push(parent.to_owned());
            }
            if !components.iter().all(|component| valid_name(component)) {
                continue;
            }
            if components.join(":") != suffix {
                continue;
            }
            let content = fs::read_to_string(&path)?;
            let prompt = plugin_command_body(&content, args, &plugin_root, session_id)?;
            if prompt.is_none() && session_id.is_some() {
                bail!("Plugin command is not user invocable")
            }
            return command_with_user_config(prompt, &manifest, &plugin_root, session_id);
        }
    }
    Ok(None)
}

/// Discover commands that can actually be resolved by command_prompt.
/// Keep the /commands view aligned with executable plugin command paths.
pub fn list_commands(root: &Path, enabled: &[String]) -> Result<Vec<String>> {
    let mut names = std::collections::BTreeSet::new();
    for plugin in enabled {
        if plugin_blocked_by_policy(root, plugin)? {
            continue;
        }
        let location = directory(root, plugin)?;
        let manifest = read_manifest(&location, plugin)?;
        let mut candidates = skill_files(&location, &manifest)?
            .into_iter().map(|(name, _)| name).collect::<Vec<_>>();
        match &manifest.commands {
            Value::Object(entries) => {
                candidates.extend(entries.keys().cloned());
            }
            value => {
                let paths: Vec<String> = match value {
                    Value::Null => vec!["commands".into()],
                    Value::String(path) => vec![path.clone()],
                    Value::Array(paths) => paths.iter().map(|path| path.as_str()
                        .map(str::to_owned).context("Plugin command path must be a string"))
                        .collect::<Result<Vec<_>>>()?,
                    _ => bail!("Plugin commands must be a path, array or metadata map"),
                };
                for relative in paths {
                    let candidate = location.join(&relative);
                    if !candidate.exists() {
                        if value.is_null() { continue; }
                        bail!("Plugin command path does not exist: {relative}")
                    }
                    let candidate = checked_component(&location, &candidate)?;
                    let paths = if candidate.is_file() {
                        vec![candidate.clone()]
                    } else {
                        walkdir::WalkDir::new(&candidate).follow_links(false)
                            .into_iter().map(|entry| entry.map_err(anyhow::Error::from))
                            .collect::<Result<Vec<_>>>()?
                            .into_iter().filter(|entry| entry.file_type().is_file())
                            .map(|entry| entry.into_path()).collect::<Vec<_>>()
                    };
                    for path in paths {
                        if !path.extension().and_then(|ext| ext.to_str())
                            .is_some_and(|ext| ext.eq_ignore_ascii_case("md")) {
                            continue;
                        }
                        let relative_path = path.strip_prefix(&candidate).unwrap_or(path.as_path());
                        let skill = path.file_name().and_then(|name| name.to_str())
                            .is_some_and(|name| name.eq_ignore_ascii_case("SKILL.md"));
                        let mut parts = Vec::new();
                        if let Some(parent) = relative_path.parent() {
                            for component in parent.components() {
                                let part = component.as_os_str().to_str()
                                    .context("Invalid plugin command namespace")?;
                                if !part.is_empty() { parts.push(part.to_owned()); }
                            }
                        }
                        if !skill {
                            let stem = path.file_stem().and_then(|stem| stem.to_str())
                                .context("Invalid plugin command name")?;
                            parts.push(stem.to_owned());
                        } else if parts.is_empty() {
                            let parent = path.parent().and_then(|parent| parent.file_name())
                                .and_then(|name| name.to_str())
                                .context("Invalid skill command directory")?;
                            parts.push(parent.to_owned());
                        }
                        if parts.iter().all(|part| valid_name(part)) {
                            candidates.push(parts.join(":"));
                        }
                    }
                }
            }
        }
        for candidate in candidates {
            let name = format!("{plugin}:{candidate}");
            if command_prompt(root, enabled, &name, "")?.is_some() {
                names.insert(name);
            }
        }
    }
    Ok(names.into_iter().collect())
}


/// SDK initialize advertises the actual metadata of user-invocable plugin
/// commands and skills. Read the same manifest paths used by execution;
/// never synthesize descriptions for files the user cannot invoke.
pub fn command_descriptors(root: &Path, enabled: &[String]) -> Result<Vec<Value>> {
    let mut descriptors = Vec::new();
    for name in list_commands(root, enabled)? {
        let (plugin, suffix) = name.split_once(':')
            .context("Invalid plugin command name")?;
        let location = directory(root, plugin)?;
        let manifest = read_manifest(&location, plugin)?;
        let source = if let Some((_, path)) = skill_files(&location, &manifest)?
            .into_iter().find(|(candidate, _)| candidate == suffix) {
            Some(fs::read_to_string(path)?)
        } else if let Some(spec) = manifest.commands.as_object()
            .and_then(|entries| entries.get(suffix)) {
            if let Some(content) = spec.get("content").and_then(Value::as_str) {
                Some(content.to_owned())
            } else if let Some(relative) = spec.get("source").and_then(Value::as_str) {
                Some(fs::read_to_string(checked_component(
                    &location, &location.join(relative),
                )?)?)
            } else {
                bail!("Plugin command requires source or content")
            }
        } else {
            let paths = match &manifest.commands {
                Value::Null => vec!["commands".to_owned()],
                Value::String(path) => vec![path.clone()],
                Value::Array(paths) => paths.iter()
                    .map(|value| value.as_str().map(str::to_owned)
                        .context("Plugin command path must be a string"))
                    .collect::<Result<Vec<_>>>()?,
                Value::Object(_) => Vec::new(),
                _ => bail!("Plugin commands must be a path, array or metadata map"),
            };
            let mut matched = None;
            for relative in paths {
                let candidate = location.join(relative);
                if !candidate.exists() && manifest.commands.is_null() {
                    continue;
                }
                let candidate = checked_component(&location, &candidate)?;
                let files = if candidate.is_file() {
                    vec![candidate.clone()]
                } else {
                    walkdir::WalkDir::new(&candidate).follow_links(false)
                        .into_iter().map(|entry| entry.map_err(anyhow::Error::from))
                        .collect::<Result<Vec<_>>>()?
                        .into_iter().filter(|entry| entry.file_type().is_file())
                        .map(|entry| entry.into_path()).collect::<Vec<_>>()
                };
                for path in files {
                    if !path.extension().and_then(|ext| ext.to_str())
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("md")) {
                        continue;
                    }
                    let relative_path = path.strip_prefix(&candidate).unwrap_or(path.as_path());
                    let skill = path.file_name().and_then(|value| value.to_str())
                        .is_some_and(|value| value.eq_ignore_ascii_case("SKILL.md"));
                    if !skill && path.parent()
                        .is_some_and(|parent| parent.join("SKILL.md").is_file()) {
                        continue;
                    }
                    let mut parts = Vec::new();
                    if let Some(parent) = relative_path.parent() {
                        for component in parent.components() {
                            let part = component.as_os_str().to_str()
                                .context("Invalid plugin command namespace")?;
                            if !part.is_empty() { parts.push(part.to_owned()); }
                        }
                    }
                    if !skill {
                        parts.push(path.file_stem().and_then(|stem| stem.to_str())
                            .context("Invalid plugin command name")?.to_owned());
                    } else if parts.is_empty() {
                        parts.push(path.parent().and_then(Path::file_name)
                            .and_then(|part| part.to_str())
                            .context("Invalid skill command directory")?.to_owned());
                    }
                    if parts.join(":") == suffix {
                        matched = Some(fs::read_to_string(checked_component(
                            &location, &path,
                        )?)?);
                        break;
                    }
                }
                if matched.is_some() { break; }
            }
            matched
        };
        let content = source.with_context(|| format!(
            "Plugin command {name} was advertised but its source disappeared"
        ))?;
        let (description, hint) = crate::skill_arguments::command_metadata(
            &content, "Installed plugin command or skill",
        );
        descriptors.push(serde_json::json!({
            "name":name,"description":description,"argumentHint":hint
        }));
    }
    Ok(descriptors)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plugin_command_respects_user_invocable_and_expands_root() {
        let root = std::path::Path::new("/tmp/plugin-root");
        let hidden = "---\nuser-invocable: false\n---\nDo not expose";
        assert_eq!(plugin_command_body(hidden, "", root, None).unwrap(), None);
        let visible = "---\nuser-invocable: true\n---\nRead ${CLAUDE_PLUGIN_ROOT}/README.md for $ARGUMENTS";
        assert_eq!(plugin_command_body(visible, "notes", root, None).unwrap().as_deref(),
            Some("Read /tmp/plugin-root/README.md for notes"));
        let nested = "---\nmetadata:\n  user-invocable: false\n---\nVisible";
        assert_eq!(plugin_command_body(nested, "", root, None).unwrap().as_deref(), Some("Visible"));
        let session = "Session ${CLAUDE_SESSION_ID}";
        assert_eq!(plugin_command_body(session, "", root, Some("session-123")).unwrap().as_deref(),
            Some("Session session-123"));
        let temporary = tempfile::tempdir().unwrap();
        let installed = temporary.path().join(".atlas/plugins/demo");
        fs::create_dir_all(&installed).unwrap();
        let data_prompt = plugin_command_body(
            "Persistent: ${CLAUDE_PLUGIN_DATA}", "", &installed, Some("s"),
        ).unwrap().unwrap();
        assert!(data_prompt.contains(".atlas/plugin-data/demo"), "{data_prompt}");
        assert!(temporary.path().join(".atlas/plugin-data/demo").is_dir());
    }
    #[test]
    fn plugin_skills_are_real_slash_commands_and_respect_visibility() {
        let root = tempfile::tempdir().unwrap();
        let plugin = root.path().join(".atlas/plugins/demo");
        let skill = plugin.join("extra/group/review");
        fs::create_dir_all(&skill).unwrap();
        fs::write(plugin.join("plugin.json"), r#"{"name":"demo","skills":"extra"}"#).unwrap();
        fs::write(skill.join("SKILL.md"),
            "---\narguments: topic\n---\nReview $topic at ${CLAUDE_SKILL_DIR}").unwrap();
        let enabled = vec!["demo".to_owned()];
        assert_eq!(list_commands(root.path(), &enabled).unwrap(),
            vec!["demo:group:review".to_owned()]);
        let prompt = command_prompt_with_session(root.path(), &enabled,
            "demo:group:review", "errors", Some("session-42")).unwrap().unwrap();
        assert!(prompt.contains("Base directory for this skill:"), "{prompt}");
        assert!(prompt.contains("Review errors at "), "{prompt}");
        assert!(prompt.contains("extra/group/review"), "{prompt}");
        fs::write(skill.join("SKILL.md"),
            "---\nuser-invocable: false\n---\nModel-only skill").unwrap();
        assert!(list_commands(root.path(), &enabled).unwrap().is_empty());
        assert!(command_prompt_with_session(root.path(), &enabled,
            "demo:group:review", "", Some("session-42")).is_err());
    }

    #[test]
    fn loads_only_explicitly_enabled_plugins() {
        let root = tempfile::tempdir().unwrap();
        let plugin = root.path().join(".atlas/plugins/formatter");
        fs::create_dir_all(plugin.join("skills/check")).unwrap();
        fs::write(
            plugin.join("plugin.json"),
            r#"{
            "name":"formatter","version":"1.0",
            "hooks":[{"event":"Stop","command":"true"}],
            "mcp_servers":[{"id":"tools","command":"some-server"}]
        }"#,
        )
        .unwrap();
        fs::write(plugin.join("skills/check/SKILL.md"), "Check formatting").unwrap();
        let disabled = PluginSet::load(root.path(), &[]).unwrap();
        assert!(disabled.hooks.is_empty());
        let enabled = vec!["formatter".to_owned()];
        let set = PluginSet::load(root.path(), &enabled).unwrap();
        assert_eq!(set.hooks.len(), 1);
        assert_eq!(set.mcp_servers[0].id, "formatter_tools");
        assert_eq!(
            list_skills(root.path(), &enabled).unwrap(),
            vec!["formatter:check"]
        );
        assert!(read_skill(root.path(), &enabled, "formatter:check")
            .unwrap()
            .contains("Check formatting"));
        assert!(read_skill(root.path(), &[], "formatter:check").is_err());
    }
    #[cfg(unix)]
    #[test]
    fn refuses_symlink_plugin_outside_workspace() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join(".atlas/plugins")).unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join(".atlas/plugins/leak"))
            .unwrap();
        assert!(PluginSet::load(root.path(), &["leak".into()]).is_err());
    }
}
