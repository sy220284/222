//! Native plugin installation and management for source command integration.
//! All user-selected local plugin files are copied as real files, loaded
//! through the same manifest loader as the agent, and rollback on failure.
use crate::{config::Config, plugin_commands::PluginCommand, plugins::PluginSet};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};
use uuid::Uuid;
use walkdir::WalkDir;

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-'))
}

fn installed_root(root: &Path) -> PathBuf {
    root.join(".atlas/plugins")
}

pub fn installed(root: &Path, config: &Config) -> Result<Value> {
    let dir = installed_root(root);
    let mut names = Vec::new();
    if dir.exists() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if !valid_name(&name) || !entry.path().is_dir() {
                continue;
            }
            let manifest = entry.path().join("plugin.json");
            if manifest.is_file() {
                names.push(json!({
                    "name":name,
                    "enabled":config.enabled_plugins.contains(&name),
                }));
            }
        }
    }
    names.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    Ok(json!({"plugins":names}))
}

fn manifest_name(root: &Path) -> Result<String> {
    let source = root.join("plugin.json");
    let document: Value =
        serde_json::from_slice(&fs::read(source).context("Local plugin missing plugin.json")?)?;
    let name = document["name"]
        .as_str()
        .context("Plugin manifest must contain a name")?;
    if !valid_name(name) {
        bail!("Invalid plugin ID in manifest");
    }
    Ok(name.to_owned())
}

fn copy_local_tree(source: &Path, stage: &Path) -> Result<()> {
    for entry in WalkDir::new(source).follow_links(false) {
        let entry = entry?;
        if entry.file_type().is_symlink() {
            bail!("Cannot install a plugin containing symbolic links");
        }
        let relative = entry.path().strip_prefix(source)?;
        let target = stage.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)?;
        } else if entry.file_type().is_file() {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

pub fn install_local(root: &Path, location: &Path) -> Result<String> {
    let source = location
        .canonicalize()
        .with_context(|| format!("Cannot access plugin source {}", location.display()))?;
    if !source.is_dir() {
        bail!("Local plugin source must be a directory");
    }
    let name = manifest_name(&source)?;
    let home = installed_root(root);
    fs::create_dir_all(&home)?;
    let target = home.join(&name);
    if target.exists() {
        bail!("Plugin '{name}' is already installed");
    }
    let stage = home.join(format!(".install-{}", Uuid::new_v4()));
    let result = (|| -> Result<()> {
        copy_local_tree(&source, &stage)?;
        fs::rename(&stage, &target)?;
        PluginSet::load(root, std::slice::from_ref(&name))?;
        Ok(())
    })();
    if let Err(error) = result {
        let _ = fs::remove_dir_all(&stage);
        let _ = fs::remove_dir_all(&target);
        return Err(error);
    }
    Ok(name)
}

pub fn execute(root: &Path, command: &PluginCommand) -> Result<Value> {
    let root = root.canonicalize()?;
    let mut config = Config::load()?;
    match command {
        PluginCommand::Menu | PluginCommand::Manage => installed(&root, &config),
        PluginCommand::Help => Ok(json!({
            "commands":[
                "install /absolute/path/to/plugin",
                "uninstall <plugin>",
                "enable <plugin>",
                "disable <plugin>",
                "validate <path>",
                "manage",
                "marketplace <add|remove|update|list>"
            ]
        })),
        PluginCommand::Install {
            marketplace,
            plugin,
        } => {
            let path = marketplace
                .as_deref()
                .context("Local plugin install requires a directory path")?;
            let local = if let Some(uri) = path.strip_prefix("file://") {
                let uri = reqwest::Url::parse(&format!("file://{uri}"))?;
                uri.to_file_path()
                    .map_err(|_| anyhow::anyhow!("Invalid file URL"))?
            } else if path.starts_with("http://") || path.starts_with("https://") {
                bail!("Remote plugin install requires a configured trusted marketplace");
            } else {
                PathBuf::from(path)
            };
            // Validate the requested identity before copying or activating
            // files. A mismatched --plugin must not leave an installed plugin
            // behind after reporting failure.
            if let Some(requested) = plugin {
                let actual = manifest_name(&local.canonicalize()?)?;
                if requested != &actual {
                    bail!("Requested plugin ID does not match the local manifest");
                }
            }
            let installed_name = install_local(&root, &local)?;
            // The original plugin install enables the resolved plugin in the
            // active settings scope. Installing without enabling leaves its
            // commands and skills invisible to the next agent session.
            if !config.enabled_plugins.contains(&installed_name) {
                config.enabled_plugins.push(installed_name.clone());
                if let Err(error) = config.save() {
                    // Keep installation atomic from the user's perspective.
                    let _ = fs::remove_dir_all(installed_root(&root).join(&installed_name));
                    return Err(error);
                }
            }
            Ok(json!({"installed":installed_name}))
        }
        PluginCommand::Enable { plugin } => {
            let name = plugin.as_ref().context("Plugin name is required")?;
            if !valid_name(name) {
                bail!("Invalid plugin name");
            }
            PluginSet::load(&root, std::slice::from_ref(name))?;
            if !config.enabled_plugins.contains(name) {
                config.enabled_plugins.push(name.clone());
                config.save()?;
            }
            Ok(json!({"enabled":name}))
        }
        PluginCommand::Disable { plugin } => {
            let name = plugin.as_ref().context("Plugin name is required")?;
            let previous = config.enabled_plugins.len();
            config.enabled_plugins.retain(|value| value != name);
            if config.enabled_plugins.len() != previous {
                config.save()?;
            }
            Ok(json!({"disabled":name}))
        }
        PluginCommand::Uninstall { plugin } => {
            let name = plugin.as_ref().context("Plugin name is required")?;
            if !valid_name(name) {
                bail!("Invalid plugin name");
            }
            let installed = installed_root(&root).join(name);
            if !installed.is_dir() || installed.is_symlink() {
                bail!("Plugin is not installed");
            }
            let removed = installed_root(&root).join(format!(".uninstall-{}", Uuid::new_v4()));
            fs::rename(&installed, &removed)?;
            config.enabled_plugins.retain(|value| value != name);
            if let Err(error) = config.save() {
                fs::rename(&removed, &installed)?;
                return Err(error);
            }
            fs::remove_dir_all(removed)?;
            Ok(json!({"uninstalled":name}))
        }
        PluginCommand::Validate { path } => {
            let path = path.as_deref().context("A plugin directory is required")?;
            let location = Path::new(path).canonicalize()?;
            let name = manifest_name(&location)?;
            Ok(json!({"name":name,"valid_manifest":true}))
        }
        PluginCommand::Marketplace { action, .. } => {
            bail!(
                "Marketplace command '{}' requires a trusted registry integration",
                action.as_deref().unwrap_or("menu")
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installs_actual_plugin_manifest_and_lists_skills() {
        let root = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        fs::create_dir_all(source.path().join("skills/review")).unwrap();
        fs::write(
            source.path().join("plugin.json"),
            r#"{"name":"reviewer","version":"1.0","hooks":[],"mcp_servers":[]}"#,
        )
        .unwrap();
        fs::write(
            source.path().join("skills/review/SKILL.md"),
            "Review a source tree",
        )
        .unwrap();
        let name = install_local(root.path(), source.path()).unwrap();
        assert_eq!(name, "reviewer");
        let enabled = vec![name.clone()];
        assert_eq!(
            crate::plugins::list_skills(root.path(), &enabled).unwrap(),
            vec!["reviewer:review"]
        );
        assert!(
            crate::plugins::read_skill(root.path(), &enabled, "reviewer:review")
                .unwrap()
                .contains("source tree")
        );
        assert!(installed_root(root.path())
            .join("reviewer/plugin.json")
            .exists());
        assert!(install_local(root.path(), source.path()).is_err());
    }
}
