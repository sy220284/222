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
fn mcp_file(root: &Path, relative: &str) -> Result<Value> {
    let path = checked_component(root, &root.join(relative))?;
    let document: Value = serde_json::from_slice(&fs::read(&path)?)
        .with_context(|| format!("Invalid plugin MCP file {}", path.display()))?;
    Ok(document.get("mcpServers").cloned().unwrap_or(document))
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
            let file = checked_component(root, &root.join(relative))?;
            if !visiting.insert(file.clone()) {
                bail!("Plugin MCP configuration contains a file reference cycle")
            }
            let result = merge_server_specs(root, &mcp_file(root, relative)?, servers, visiting);
            visiting.remove(&file);
            result?;
        }
        Value::Array(specs) => {
            for spec in specs {
                if let Some(id) = spec.get("id").and_then(Value::as_str) {
                    servers.insert(id.to_owned(), spec.clone());
                } else {
                    merge_server_specs(root, spec, servers, visiting)?;
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
        merge_server_specs(root, &mcp_file(root, ".mcp.json")?, &mut entries, &mut visiting)?;
    }
    merge_server_specs(root, value, &mut entries, &mut visiting)?;
    let mut servers = Vec::new();
    for (name, config) in entries {
        let mut config = config;
        let object = config.as_object_mut()
            .with_context(|| format!("Plugin MCP server {name} must be an object"))?;
        object.insert("id".into(), Value::String(name));
        servers.push(serde_json::from_value(config)?);
    }
    Ok(servers)
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
}
fn valid_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}
fn directory(root: &Path, name: &str) -> Result<PathBuf> {
    if !valid_name(name) {
        bail!("Invalid plugin id: {name}")
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
        };
        let mut seen = std::collections::HashSet::new();
        for name in enabled {
            if !seen.insert(name) {
                bail!("Plugin {name} enabled twice")
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
            for mut server in manifest_servers(&location, &manifest.mcp_servers)? {
                server.id = format!("{}_{}", name, server.id);
                if server.id.len() > 64 {
                    bail!("Plugin MCP tool prefix too long")
                }
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
        let plugin_root = directory(root, name)?;
        let manifest = read_manifest(&plugin_root, name)?;
        for (skill, _) in skill_files(&plugin_root, &manifest)? {
            names.push(format!("{name}:{skill}"));
        }
    }
    names.sort();
    Ok(names)
}
pub fn read_skill(root: &Path, enabled: &[String], identifier: &str) -> Result<String> {
    let (name, skill) = identifier
        .split_once(':')
        .context("Use plugin:skill as plugin skill name")?;
    if !skill.split(':').all(valid_name) || !enabled.iter().any(|n| n == name) {
        bail!("Plugin skill not enabled")
    }
    let plugin_root = directory(root, name)?;
    let manifest = read_manifest(&plugin_root, name)?;
    let path = skill_files(&plugin_root, &manifest)?
        .into_iter().find(|(candidate, _)| candidate == skill)
        .map(|(_, path)| path).context("Plugin skill not found")?;
    if fs::metadata(&path)?.len() > 64_000 {
        bail!("Plugin skill too large")
    }
    Ok(fs::read_to_string(path)?)
}

/// Resolve an original plugin Markdown slash command to its model-visible
/// prompt. The interactive entrypoint calls this before the ordinary user
/// prompt path, so a plugin command actually executes instead of being sent
/// as unrecognized literal text.
pub fn command_prompt(
    root: &Path, enabled: &[String], command: &str, args: &str,
) -> Result<Option<String>> {
    let Some((plugin, suffix)) = command.split_once(':') else {
        return Ok(None);
    };
    if !enabled.iter().any(|name| name == plugin)
        || !valid_name(plugin) || !suffix.split(':').all(valid_name) {
        return Ok(None);
    }
    let plugin_root = directory(root, plugin)?;
    let manifest = read_manifest(&plugin_root, plugin)?;
    if manifest.name != plugin {
        bail!("Plugin manifest identity does not match installed name")
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
            let names = crate::skill_arguments::frontmatter_argument_names(&content);
            let prompt = crate::skill_arguments::substitute_arguments(
                crate::skill_arguments::markdown_body(&content), Some(args), true, &names,
            );
            return Ok(Some(prompt));
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
            let names = crate::skill_arguments::frontmatter_argument_names(&content);
            return Ok(Some(crate::skill_arguments::substitute_arguments(
                crate::skill_arguments::markdown_body(&content), Some(args), true, &names,
            )));
        }
    }
    Ok(None)
}

/// Discover commands that can actually be resolved by command_prompt.
/// Keep the /commands view aligned with executable plugin command paths.
pub fn list_commands(root: &Path, enabled: &[String]) -> Result<Vec<String>> {
    let mut names = std::collections::BTreeSet::new();
    for plugin in enabled {
        let location = directory(root, plugin)?;
        let manifest = read_manifest(&location, plugin)?;
        let mut candidates = Vec::new();
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
                            .into_iter().filter_map(|entry| entry.ok())
                            .filter(|entry| entry.file_type().is_file())
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

#[cfg(test)]
mod tests {
    use super::*;
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
