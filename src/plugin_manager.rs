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

/// Plugin management must never follow a redirected storage directory.
/// The manifest loader checks the final plugin directory, but installation
/// writes files before loading the manifest, so validate the entire base first.
fn checked_installed_root(root: &Path, create: bool) -> Result<PathBuf> {
    let root = root.canonicalize().context("Workspace unavailable")?;
    let atlas = root.join(".atlas");
    let home = installed_root(&root);
    for path in [&atlas, &home] {
        match fs::symlink_metadata(path) {
            Ok(meta) if meta.file_type().is_symlink() => {
                bail!("Plugin storage cannot be a symbolic link: {}", path.display())
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    if create {
        fs::create_dir_all(&home)?;
    } else if !home.exists() {
        // Listing installed plugins is read-only; do not create storage.
        return Ok(home);
    }
    let real = home.canonicalize()?;
    if !real.starts_with(&root) {
        bail!("Plugin storage escapes the workspace")
    }
    Ok(real)
}

pub fn installed(root: &Path, config: &Config) -> Result<Value> {
    let dir = checked_installed_root(root, false)?;
    let mut names = Vec::new();
    if dir.exists() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if !valid_name(&name)
                || entry.file_type()?.is_symlink()
                || !entry.path().is_dir()
            {
                continue;
            }
            let location = entry.path();
            let has_components = ["commands", "agents", "skills", "hooks"]
                .iter().any(|component| location.join(component).is_dir());
            let has_manifest = location.join(".claude-plugin/plugin.json").is_file()
                || location.join("plugin.json").is_file();
            if has_manifest || has_components {
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
    crate::plugins::validate_local(root)
}

fn copy_local_tree(source: &Path, stage: &Path) -> Result<()> {
    // pluginLoader.ts::installFromLocal removes the source repository's .git
    // metadata. Do not copy private remotes, hooks or history into the
    // installed plugin in the first place.
    for entry in WalkDir::new(source).follow_links(false).into_iter()
        .filter_entry(|entry| entry.depth() != 1
            || entry.file_name() != std::ffi::OsStr::new(".git"))
    {
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
    let home = checked_installed_root(root, true)?;
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

/// Match the source uninstall UI: persistent plugin data must be disclosed
/// before the last installation is removed. Symlinks are never followed.
fn persistent_plugin_data_size(root: &Path, name: &str) -> Result<Option<u64>> {
    let base = root.join(".atlas/plugin-data");
    let metadata = match fs::symlink_metadata(&base) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        bail!("Plugin data storage is redirected or invalid");
    }
    let data = base.join(name);
    let metadata = match fs::symlink_metadata(&data) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        bail!("Plugin data directory is redirected or invalid");
    }
    let mut bytes = 0u64;
    for entry in WalkDir::new(data).follow_links(false) {
        let entry = entry?;
        if entry.file_type().is_file() {
            bytes = bytes.saturating_add(entry.metadata()?.len());
        }
    }
    Ok((bytes > 0).then_some(bytes))
}

pub fn execute(root: &Path, command: &PluginCommand) -> Result<Value> {
    let root = root.canonicalize()?;
    let mut config = Config::load()?;
    match command {
        PluginCommand::Menu | PluginCommand::Manage => installed(&root, &config),
        PluginCommand::Help => Ok(json!({
            "commands":[
                "install /absolute/path/to/plugin",
                "uninstall <plugin> [--delete-data|--keep-data]",
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
            let local = if let Some(plugin_name) = plugin {
                if let Some(marketplace) = marketplace.as_deref() {
                    crate::marketplace_registry::resolve_local_plugin(
                        &root, marketplace, plugin_name,
                    )?
                } else {
                    crate::marketplace_registry::resolve_local_plugin_by_name(
                        &root, plugin_name,
                    )?.1
                }
            } else {
                let path = marketplace.as_deref().context(
                    "Plugin name, local directory or registered marketplace is required",
                )?;
                if let Some(uri) = path.strip_prefix("file://") {
                    let uri = reqwest::Url::parse(&format!("file://{uri}"))?;
                    uri.to_file_path()
                        .map_err(|_| anyhow::anyhow!("Invalid file URL"))?
                } else if path.starts_with("http://") || path.starts_with("https://") {
                    bail!("Remote plugin install requires a configured trusted marketplace");
                } else {
                    PathBuf::from(path)
                }
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
                    // The installation base was already verified by install_local.
                    if let Ok(home) = checked_installed_root(&root, true) {
                        let _ = fs::remove_dir_all(home.join(&installed_name));
                    }
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
            if config.enabled_plugins.contains(name) {
                bail!("Plugin '{name}' is already enabled");
            }
            config.enabled_plugins.push(name.clone());
            config.save()?;
            Ok(json!({"enabled":name}))
        }
        PluginCommand::Disable { plugin } => {
            let name = plugin.as_ref().context("Plugin name is required")?;
            if !valid_name(name) {
                bail!("Invalid plugin name");
            }
            let installed = checked_installed_root(&root, false)?.join(name);
            if !installed.is_dir() || installed.is_symlink() {
                bail!("Plugin '{name}' is not installed");
            }
            if !config.enabled_plugins.contains(name) {
                bail!("Plugin '{name}' is already disabled");
            }
            config.enabled_plugins.retain(|value| value != name);
            config.save()?;
            Ok(json!({"disabled":name}))
        }
        PluginCommand::Uninstall { plugin, delete_data } => {
            let name = plugin.as_ref().context("Plugin name is required")?;
            if !valid_name(name) {
                bail!("Invalid plugin name");
            }
            let home = checked_installed_root(&root, true)?;
            let installed = home.join(name);
            if !installed.is_dir() || installed.is_symlink() {
                bail!("Plugin is not installed");
            }
            if delete_data.is_none() {
                if let Some(size) = persistent_plugin_data_size(&root, name)? {
                    return Ok(json!({
                        "confirmationRequired":true,
                        "plugin":name,
                        "dataBytes":size,
                        "choices":["--delete-data","--keep-data"]
                    }));
                }
            }
            let removed = home.join(format!(".uninstall-{}", Uuid::new_v4()));
            fs::rename(&installed, &removed)?;
            config.enabled_plugins.retain(|value| value != name);
            if let Err(error) = config.save() {
                fs::rename(&removed, &installed)?;
                return Err(error);
            }
            // The original last-scope uninstall removes persistent plugin
            // data as well as the installed files. This implementation has
            // one local scope, so uninstall always releases that data.
            // Cleanup failures are visible but must not misreport a
            // successfully committed uninstall as a failed operation.
            let mut warnings = Vec::new();
            if let Err(error) = fs::remove_dir_all(&removed) {
                warnings.push(format!("Could not clean staged plugin files: {error}"));
            }
            let data_root = root.join(".atlas/plugin-data");
            let cleanup = (|| -> Result<()> {
                if *delete_data == Some(false) {
                    return Ok(());
                }
                let metadata = match fs::symlink_metadata(&data_root) {
                    Ok(metadata) => metadata,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        return Ok(());
                    }
                    Err(error) => return Err(error.into()),
                };
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    bail!("Plugin data storage is redirected or invalid");
                }
                let data = data_root.join(name);
                match fs::symlink_metadata(&data) {
                    Ok(metadata) if metadata.file_type().is_symlink() => {
                        bail!("Plugin data directory is a symbolic link");
                    }
                    Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(&data)?,
                    Ok(_) => bail!("Plugin data directory is not a directory"),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error.into()),
                }
                Ok(())
            })();
            if let Err(error) = cleanup {
                warnings.push(format!("Could not remove persistent plugin data: {error}"));
            }
            // Plugin options are keyed by the installed identity. Delete only
            // the exact or unambiguous scoped entry; never erase another
            // marketplace's options merely because the short name matches.
            match crate::permission_settings::SettingsPaths::for_workspace(&root)
                .and_then(|settings| settings.delete_plugin_public_options(name))
            {
                Ok(_) => {}
                Err(error) => {
                    warnings.push(format!("Could not remove stored plugin options: {error}"));
                }
            }
            let preserved = *delete_data == Some(false);
            if warnings.is_empty() {
                Ok(json!({"uninstalled":name,"dataPreserved":preserved}))
            } else {
                Ok(json!({"uninstalled":name,"dataPreserved":preserved,"warnings":warnings}))
            }
        }
        PluginCommand::Validate { path } => {
            let path = path.as_deref().context("A plugin directory is required")?;
            let location = Path::new(path).canonicalize()?;
            let name = manifest_name(&location)?;
            Ok(json!({"name":name,"valid_manifest":true}))
        }
        PluginCommand::Marketplace { action, target } => {
            crate::marketplace_registry::execute(&root, action.as_deref(), target.as_deref())
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
