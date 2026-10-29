use crate::{config::McpServerConfig, hooks::HookRule};
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
    hooks: Vec<HookRule>,
    #[serde(default)]
    mcp_servers: Vec<McpServerConfig>,
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
            let manifest_path = location.join("plugin.json");
            let manifest: Manifest = serde_json::from_slice(
                &fs::read(&manifest_path)
                    .with_context(|| format!("Plugin {name} missing plugin.json"))?,
            )?;
            if manifest.name != *name {
                bail!("Plugin name does not match directory: {name}")
            }
            // Version is metadata; no external update or installation happens.
            let _version = &manifest.version;
            set.hooks.extend(manifest.hooks);
            for mut server in manifest.mcp_servers {
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
pub fn list_skills(root: &Path, enabled: &[String]) -> Result<Vec<String>> {
    let mut names = Vec::new();
    for name in enabled {
        let plugin_root = directory(root, name)?;
        let dir = plugin_root.join("skills");
        if !dir.is_dir() {
            continue;
        }
        if !dir.canonicalize()?.starts_with(&plugin_root) {
            bail!("Plugin skills directory leaves plugin root")
        }
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let skill = entry.file_name().to_string_lossy().to_string();
            if !valid_name(&skill) {
                continue;
            }
            let path = entry.path().join("SKILL.md");
            if path.is_file() {
                // Discovery and read_skill must enforce the same plugin
                // boundary. A link into another installed plugin could
                // otherwise appear in the list but fail when opened.
                if !path.canonicalize()?.starts_with(&plugin_root) {
                    bail!("Plugin skill path leaves plugin root")
                }
                names.push(format!("{name}:{skill}"));
            }
        }
    }
    names.sort();
    Ok(names)
}
pub fn read_skill(root: &Path, enabled: &[String], identifier: &str) -> Result<String> {
    let (name, skill) = identifier
        .split_once(':')
        .context("Use plugin:skill as plugin skill name")?;
    if !valid_name(skill) || !enabled.iter().any(|n| n == name) {
        bail!("Plugin skill not enabled")
    }
    let dir = directory(root, name)?;
    let path = dir
        .join("skills")
        .join(skill)
        .join("SKILL.md")
        .canonicalize()?;
    if !path.starts_with(&dir) {
        bail!("Plugin skill path leaves plugin root")
    }
    if fs::metadata(&path)?.len() > 64_000 {
        bail!("Plugin skill too large")
    }
    Ok(fs::read_to_string(path)?)
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
