//! Manual port of 2.1.88 utils/plugins/lspPluginIntegration.ts.
//! A bad optional plugin or server cannot suppress healthy LSP integrations.
use crate::config::LspServerConfig;
use anyhow::{bail, Context, Result};
use serde_json::{Map, Value};
use std::{collections::BTreeMap, fs, path::Path};

fn safe_config_path(root: &Path, relative: &str) -> Result<std::path::PathBuf> {
    let input = Path::new(relative);
    if input.is_absolute()
        || input
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        bail!("Plugin LSP config must use a relative path within the plugin")
    }
    let resolved = root
        .join(input)
        .canonicalize()
        .context("Cannot open plugin LSP config")?;
    if !resolved.starts_with(root.canonicalize()?) || !resolved.is_file() {
        bail!("Plugin LSP config leaves plugin root or is not a file")
    }
    Ok(resolved)
}

fn read_json_map(root: &Path, file: &str) -> Result<Map<String, Value>> {
    let path = safe_config_path(root, file)?;
    let content = fs::read(&path).with_context(|| format!("Read {}", path.display()))?;
    if content.len() > 1024 * 1024 {
        bail!("Plugin LSP config is larger than 1 MiB")
    }
    let data: Value = serde_json::from_slice(&content)
        .with_context(|| format!("Invalid JSON in {}", path.display()))?;
    data.as_object()
        .cloned()
        .context("Plugin LSP config must be a server map")
}

fn expand_field(
    raw: &str,
    plugin_root: &Path,
    plugin_data: &Path,
    options: &Map<String, Value>,
    schema: &Value,
) -> Result<String> {
    let mut output = String::new();
    let mut remainder = raw;
    while let Some(start) = remainder.find("${") {
        output.push_str(&remainder[..start]);
        let suffix = &remainder[start + 2..];
        let end = suffix
            .find('}')
            .context("Unclosed LSP environment variable")?;
        let key = &suffix[..end];
        let value = match key {
            "ATLAS_PLUGIN_ROOT" => plugin_root.to_string_lossy().into_owned(),
            "ATLAS_PLUGIN_DATA" => plugin_data.to_string_lossy().into_owned(),
            _ if key.starts_with("user_config.") => {
                let option = &key["user_config.".len()..];
                let field = schema.get(option).with_context(|| {
                    format!("Undeclared plugin LSP user configuration: {option}")
                })?;
                if field.get("sensitive").and_then(Value::as_bool) == Some(true) {
                    bail!("Plugin LSP configuration '{option}' requires secure secret storage")
                }
                let value = options
                    .get(option)
                    .with_context(|| format!("Missing plugin LSP user configuration: {option}"))?;
                match value {
                    Value::String(s) => s.clone(),
                    Value::Bool(b) => b.to_string(),
                    Value::Number(n) => n.to_string(),
                    _ => bail!("Invalid plugin LSP user configuration type for {option}"),
                }
            }
            _ => std::env::var(key)
                .with_context(|| format!("Missing environment variable for plugin LSP: {key}"))?,
        };
        output.push_str(&value);
        remainder = &suffix[end + 1..];
    }
    output.push_str(remainder);
    Ok(output)
}

fn parse_server(
    name: &str,
    spec: &Value,
    plugin: &str,
    root: &Path,
    data: &Path,
    options: &Map<String, Value>,
    schema: &Value,
) -> Result<LspServerConfig> {
    let map = spec
        .as_object()
        .context("Plugin LSP server must be an object")?;
    const FIELDS: &[&str] = &[
        "command",
        "args",
        "extensionToLanguage",
        "transport",
        "env",
        "initializationOptions",
        "settings",
        "workspaceFolder",
        "startupTimeout",
        "shutdownTimeout",
        "restartOnCrash",
        "maxRestarts",
    ];
    if let Some(field) = map.keys().find(|key| !FIELDS.contains(&key.as_str())) {
        bail!("Unsupported plugin LSP setting: {field}")
    }
    let command = map
        .get("command")
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty())
        .context("Plugin LSP requires command")?;
    if command.contains(' ') && !command.starts_with('/') {
        bail!("Plugin LSP command contains spaces; use args instead")
    }
    let transport = map
        .get("transport")
        .and_then(Value::as_str)
        .unwrap_or("stdio");
    if transport != "stdio" {
        bail!("Plugin LSP socket transport requires a socket transport implementation")
    }
    let mappings = map
        .get("extensionToLanguage")
        .and_then(Value::as_object)
        .filter(|v| !v.is_empty())
        .context("Plugin LSP requires nonempty extensionToLanguage")?;
    let mut extension_to_language = std::collections::HashMap::new();
    for (extension, language) in mappings {
        if !extension.starts_with('.') || extension.len() < 2 {
            bail!("Plugin LSP file extensions must begin with .")
        }
        let language = language
            .as_str()
            .filter(|v| !v.is_empty())
            .context("Plugin LSP language ID must be a nonempty string")?;
        extension_to_language.insert(
            extension.trim_start_matches('.').to_owned(),
            language.to_owned(),
        );
    }
    let raw_args: Vec<String> = match map.get("args") {
        None => Vec::new(),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .context("Plugin LSP args must be nonempty strings")
            })
            .collect::<Result<Vec<_>>>()?,
        _ => bail!("Plugin LSP args must be an array"),
    };
    let raw_env: Map<String, Value> = match map.get("env") {
        None => Map::new(),
        Some(Value::Object(map)) => map.clone(),
        _ => bail!("Plugin LSP env must be an object"),
    };
    let mut env = std::collections::HashMap::new();
    for (key, raw) in raw_env {
        let text = raw
            .as_str()
            .context("Plugin LSP env values must be strings")?;
        env.insert(key, expand_field(text, root, data, options, schema)?);
    }
    env.insert(
        "ATLAS_PLUGIN_ROOT".into(),
        root.to_string_lossy().into_owned(),
    );
    env.insert(
        "ATLAS_PLUGIN_DATA".into(),
        data.to_string_lossy().into_owned(),
    );
    let args = raw_args
        .iter()
        .map(|arg| expand_field(arg, root, data, options, schema))
        .collect::<Result<Vec<_>>>()?;
    let workspace_folder = map
        .get("workspaceFolder")
        .and_then(Value::as_str)
        .map(|text| expand_field(text, root, data, options, schema))
        .transpose()?;
    let timeout = map
        .get("startupTimeout")
        .map(|n| {
            n.as_u64()
                .filter(|n| *n > 0)
                .context("Plugin LSP startupTimeout must be a positive integer")
        })
        .transpose()?;
    let shutdown = map
        .get("shutdownTimeout")
        .map(|n| {
            n.as_u64()
                .filter(|n| *n > 0)
                .context("Plugin LSP shutdownTimeout must be a positive integer")
        })
        .transpose()?;
    let max_restarts = map
        .get("maxRestarts")
        .map(|n| {
            n.as_u64()
                .context("Plugin LSP maxRestarts must be a nonnegative integer")
        })
        .transpose()?;
    let restart_on_crash = map
        .get("restartOnCrash")
        .map(|v| {
            v.as_bool()
                .context("Plugin LSP restartOnCrash must be boolean")
        })
        .transpose()?;
    let mut extensions = extension_to_language.keys().cloned().collect::<Vec<_>>();
    extensions.sort();
    Ok(LspServerConfig {
        id: format!("plugin:{plugin}:{name}"),
        command: expand_field(command, root, data, options, schema)?,
        args,
        extensions,
        language_id: String::new(),
        extension_to_language,
        env,
        initialization_options: map.get("initializationOptions").cloned(),
        settings: map.get("settings").cloned(),
        workspace_folder,
        startup_timeout: timeout,
        shutdown_timeout: shutdown,
        restart_on_crash,
        max_restarts,
    })
}

fn merge_server_entry(
    specs: &mut BTreeMap<String, Value>,
    errors: &mut Vec<String>,
    plugin: &str,
    entry: &str,
    data: Result<Map<String, Value>>,
) {
    match data {
        Ok(items) => {
            for (name, config) in items {
                specs.insert(name, config);
            }
        }
        Err(error) => errors.push(format!("Plugin '{plugin}' LSP config '{entry}': {error:#}")),
    }
}

/// Read optional .lsp.json, then manifest inline/files in declaration order,
/// where later definitions replace earlier servers of the same local name.
pub fn load_plugin_lsp(
    root: &Path,
    plugin: &str,
    declaration: &Value,
    user_schema: &Value,
    options: &Map<String, Value>,
) -> (Vec<LspServerConfig>, Vec<String>) {
    let mut specs = BTreeMap::new();
    let mut errors = Vec::new();
    let file = root.join(".lsp.json");
    if file.exists() || file.is_symlink() {
        merge_server_entry(
            &mut specs,
            &mut errors,
            plugin,
            ".lsp.json",
            read_json_map(root, ".lsp.json"),
        );
    }
    let definitions: Vec<&Value> = match declaration {
        Value::Null => Vec::new(),
        Value::Array(values) => values.iter().collect(),
        other => vec![other],
    };
    for value in definitions {
        match value {
            Value::String(file) => merge_server_entry(
                &mut specs,
                &mut errors,
                plugin,
                file,
                read_json_map(root, file),
            ),
            Value::Object(map) => {
                merge_server_entry(&mut specs, &mut errors, plugin, "inline", Ok(map.clone()))
            }
            _ => errors.push(format!(
                "Plugin '{plugin}' LSP declaration must be map, path or array"
            )),
        }
    }
    let data = root
        .parent()
        .and_then(Path::parent)
        .map(|directory| directory.join("plugin-data").join(plugin));
    let mut servers = Vec::new();
    let Some(data) = data else {
        errors.push(format!("Plugin '{plugin}' has invalid storage location"));
        return (servers, errors);
    };
    if data.is_symlink() || data.parent().is_some_and(Path::is_symlink) {
        errors.push(format!("Plugin '{plugin}' data directory is redirected"));
        return (servers, errors);
    }
    // A live language server may write to its plugin data directory.
    // Enforce the boundary before creating it, rather than passing a path
    // that does not exist to the subprocess.
    if let Err(error) = fs::create_dir_all(&data) {
        errors.push(format!(
            "Plugin '{plugin}' LSP data directory unavailable: {error}"
        ));
        return (servers, errors);
    }
    for (name, spec) in specs {
        match parse_server(&name, &spec, plugin, root, &data, options, user_schema) {
            Ok(server) => servers.push(server),
            Err(error) => errors.push(format!("Plugin '{plugin}' LSP server '{name}': {error:#}")),
        }
    }
    (servers, errors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn loads_file_manifest_override_scoped_names_and_languages() {
        let root = tempfile::tempdir().unwrap();
        let plugin_root = root.path().join(".atlas/plugins/typescript");
        fs::create_dir_all(&plugin_root).unwrap();
        fs::write(
            plugin_root.join(".lsp.json"),
            json!({
                "ts":{"command":"first","extensionToLanguage":{".ts":"typescript"}}
            })
            .to_string(),
        )
        .unwrap();
        let decl = json!([{"ts":{"command":"later","extensionToLanguage":{".ts":"typescript",".tsx":"typescriptreact"}}}]);
        let (servers, errors) =
            load_plugin_lsp(&plugin_root, "typescript", &decl, &Value::Null, &Map::new());
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].command, "later");
        assert_eq!(servers[0].id, "plugin:typescript:ts");
        assert_eq!(servers[0].extension_to_language["tsx"], "typescriptreact");
    }
    #[test]
    fn rejects_path_escape_and_preserves_healthy_entries() {
        let root = tempfile::tempdir().unwrap();
        let plugin_root = root.path().join(".atlas/plugins/rust");
        fs::create_dir_all(&plugin_root).unwrap();
        let decl = json!([
            "../outside.json",
            {"valid":{"command":"ok","extensionToLanguage":{".rs":"rust"}},
             "broken":{"command":"","extensionToLanguage":{".rs":"rust"}}}
        ]);
        let (servers, errors) =
            load_plugin_lsp(&plugin_root, "rust", &decl, &Value::Null, &Map::new());
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].id, "plugin:rust:valid");
        assert_eq!(errors.len(), 2);
    }
}
