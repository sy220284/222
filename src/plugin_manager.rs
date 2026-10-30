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

/// Serialize plugin mutations across processes before reading the shared
/// configuration. The lock file is intentionally retained: unlinking it
/// would allow two processes to lock different inodes for the same settings.
fn plugin_operation_lock() -> Result<fs::File> {
    let config_path = Config::path()?;
    let parent = config_path.parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .with_context(|| format!("Cannot create plugin settings directory {}", parent.display()))?;
    let lock_path = parent.join(".atlas-plugin-operations.lock");
    match fs::symlink_metadata(&lock_path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            bail!("Plugin settings lock is redirected or invalid: {}", lock_path.display());
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let file = options.open(&lock_path)
        .with_context(|| format!("Cannot open plugin settings lock {}", lock_path.display()))?;
    if !file.metadata()?.is_file() {
        bail!("Plugin settings lock is not a regular file");
    }
    fs2::FileExt::lock_exclusive(&file)
        .with_context(|| format!("Cannot acquire plugin settings lock {}", lock_path.display()))?;
    Ok(file)
}

use uuid::Uuid;
use walkdir::WalkDir;

fn valid_name(name: &str) -> bool {
    // Marketplace plugin identifiers allow dotted names and have no
    // arbitrary 64-byte limit. Requiring an ASCII alphanumeric first
    // component keeps "."/".." and path separators out of storage paths.
    let mut bytes = name.bytes();
    matches!(bytes.next(), Some(c) if c.is_ascii_alphanumeric())
        && bytes.all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.'))
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
    let (active, runtime_errors) = crate::plugins::active_plugin_names(root, &config.enabled_plugins);
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
                let enabled = config.enabled_plugins.contains(&name);
                let prefix = format!("Plugin '{name}' ");
                let warnings = runtime_errors.iter()
                    .filter(|message| message.starts_with(&prefix))
                    .cloned().collect::<Vec<_>>();
                names.push(json!({
                    "name":name,
                    "enabled":enabled,
                    "active":active.contains(&name),
                    "warnings":warnings,
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

fn installed_origin_id(target: &Path) -> Result<Option<String>> {
    let origin_file = target.join(".atlas-plugin-origin.json");
    match fs::symlink_metadata(&origin_file) {
        Ok(meta) if meta.is_file() && !meta.file_type().is_symlink() => {
            let value: Value = serde_json::from_slice(&fs::read(&origin_file)?)
                .with_context(|| format!("Invalid plugin marketplace identity at {}", origin_file.display()))?;
            let id = value.get("pluginId").and_then(Value::as_str)
                .context("Installed plugin has invalid marketplace identity")?;
            let (name, marketplace) = id.split_once('@')
                .context("Installed plugin marketplace identity must contain @")?;
            if !valid_name(name) || marketplace.is_empty()
                || target.file_name().and_then(|part| part.to_str()) != Some(name) {
                bail!("Installed plugin has invalid or mismatched marketplace identity '{id}'");
            }
            Ok(Some(id.to_owned()))
        }
        Ok(_) => bail!("Installed plugin marketplace identity is redirected or invalid"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}


/// Rebase a link to the corresponding staged file without dereferencing
/// the plugin source. Both paths share the same staging root.
fn relative_plugin_link(from: &Path, to: &Path) -> PathBuf {
    let from_parts = from.components().collect::<Vec<_>>();
    let to_parts = to.components().collect::<Vec<_>>();
    let mut common = 0;
    while common < from_parts.len() && common < to_parts.len()
        && from_parts[common] == to_parts[common] {
        common += 1;
    }
    let mut result = PathBuf::new();
    for _ in common..from_parts.len() {
        result.push("..");
    }
    for part in &to_parts[common..] {
        result.push(part.as_os_str());
    }
    if result.as_os_str().is_empty() {
        result.push(".");
    }
    result
}

fn copy_local_tree(source: &Path, stage: &Path) -> Result<()> {
    // pluginLoader.ts::copyDir preserves symlinks: internal links are rebased
    // to the installed copy, external links become absolute, and broken links
    // retain their original text. WalkDir never follows directory symlinks.
    // installFromLocal excludes the source .git directory.
    for entry in WalkDir::new(source).follow_links(false).into_iter()
        .filter_entry(|entry| entry.depth() != 1
            || entry.file_name() != std::ffi::OsStr::new(".git"))
    {
        let entry = entry?;
        let relative = entry.path().strip_prefix(source)?;
        let target = stage.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)?;
        } else if entry.file_type().is_file() {
            fs::copy(entry.path(), target)?;
        } else if entry.file_type().is_symlink() {
            let raw_link = fs::read_link(entry.path())?;
            let resolved = entry.path().canonicalize().ok();
            let link = match resolved.as_ref() {
                Some(real) if real.starts_with(source) => {
                    let staged_target = stage.join(real.strip_prefix(source)?);
                    relative_plugin_link(
                        target.parent().context("Plugin link has no parent")?,
                        &staged_target,
                    )
                }
                Some(real) => real.to_owned(),
                None => raw_link,
            };
            #[cfg(unix)]
            std::os::unix::fs::symlink(&link, &target)?;
            #[cfg(windows)]
            {
                if resolved.as_ref().is_some_and(|path| path.is_dir()) {
                    std::os::windows::fs::symlink_dir(&link, &target)?;
                } else {
                    std::os::windows::fs::symlink_file(&link, &target)?;
                }
            }
            #[cfg(not(any(unix, windows)))]
            bail!("Plugin symbolic links are unsupported on this platform");
        }
    }
    Ok(())
}

pub fn install_local(root: &Path, location: &Path) -> Result<String> {
    let _lock = plugin_operation_lock()?;
    install_local_with_origin(root, location, None)
}

fn install_local_with_origin(
    root: &Path, location: &Path, marketplace_id: Option<&str>,
) -> Result<String> {
    let source = location
        .canonicalize()
        .with_context(|| format!("Cannot access plugin source {}", location.display()))?;
    if !source.is_dir() {
        bail!("Local plugin source must be a directory");
    }
    let name = manifest_name(&source)?;
    let home = checked_installed_root(root, true)?;
    // The staging directory is created under this storage root. Copying a
    // source that contains it would recursively copy the growing stage.
    if home.starts_with(&source) {
        bail!("Plugin source contains the installation storage directory");
    }
    fs::create_dir_all(&home)?;
    let target = home.join(&name);
    if target.exists() {
        bail!("Plugin '{name}' is already installed");
    }
    let stage = home.join(format!(".install-{}", Uuid::new_v4()));
    let mut activated = false;
    let result = (|| -> Result<()> {
        copy_local_tree(&source, &stage)?;
        let origin = stage.join(".atlas-plugin-origin.json");
        if origin.exists() {
            bail!("Plugin source contains reserved installation metadata");
        }
        if let Some(plugin_id) = marketplace_id {
            fs::write(&origin, serde_json::to_vec(&json!({"pluginId":plugin_id}))?)?;
        }
        fs::rename(&stage, &target)?;
        activated = true;
        if crate::plugins::plugin_blocked_by_policy(root, &name)? {
            bail!("Plugin '{name}' is disabled by managed policy");
        }
        PluginSet::load(root, std::slice::from_ref(&name))?;
        Ok(())
    })();
    if let Err(error) = result {
        // A failed stage or rename must never erase another installation.
        // Only remove the final directory after our own rename succeeded.
        if stage.exists() {
            fs::remove_dir_all(&stage)
                .with_context(|| format!("Plugin install failed: {error:#}; staged cleanup failed"))?;
        }
        if activated {
            fs::remove_dir_all(&target)
                .with_context(|| format!("Plugin install failed: {error:#}; activated rollback failed"))?;
        }
        return Err(error);
    }
    Ok(name)
}

/// The original dependency resolver always materializes the requested root,
/// even when it is already enabled. Stage a replacement before touching the
/// installed copy, then retain the previous directory until settings commit.
fn replace_local_with_origin(
    root: &Path, location: &Path, marketplace_id: Option<&str>,
) -> Result<PathBuf> {
    let source = location.canonicalize()
        .with_context(|| format!("Cannot access plugin source {}", location.display()))?;
    if !source.is_dir() { bail!("Plugin source must be a directory"); }
    let name = manifest_name(&source)?;
    let home = checked_installed_root(root, false)?;
    if home.starts_with(&source) {
        bail!("Plugin source contains the installation storage directory");
    }
    let target = home.join(&name);
    let metadata = fs::symlink_metadata(&target)
        .with_context(|| format!("Plugin '{name}' is not installed"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        bail!("Installed plugin '{name}' is redirected or invalid");
    }
    let existing_origin = installed_origin_id(&target)?;
    if existing_origin.as_deref() != marketplace_id {
        bail!("Cannot replace plugin '{name}' from a different marketplace or source");
    }
    let origin_blocked = marketplace_id.map(|id|
        crate::plugins::plugin_blocked_by_policy(root, id)).transpose()?.unwrap_or(false);
    if crate::plugins::plugin_blocked_by_policy(root, &name)? || origin_blocked {
        bail!("Plugin '{name}' is disabled by managed policy");
    }
    let stage = home.join(format!(".update-{}", Uuid::new_v4()));
    let backup = home.join(format!(".previous-{}", Uuid::new_v4()));
    let staged = (|| -> Result<()> {
        // The temporary directory starts with a dot, so manifest-less plugins
        // cannot be validated using its temporary basename. The source was
        // fully validated by manifest_name before the byte-for-byte copy.
        copy_local_tree(&source, &stage)?;
        let origin = stage.join(".atlas-plugin-origin.json");
        if origin.exists() {
            bail!("Plugin source contains reserved installation metadata");
        }
        if let Some(id) = marketplace_id {
            fs::write(&origin, serde_json::to_vec(&json!({"pluginId":id}))?)?;
        }
        Ok(())
    })();
    if let Err(error) = staged {
        if stage.exists() {
            fs::remove_dir_all(&stage).context("Cannot clean failed plugin update")?;
        }
        return Err(error);
    }
    if let Err(error) = fs::rename(&target, &backup) {
        fs::remove_dir_all(&stage).context("Cannot clean staged plugin after update failure")?;
        return Err(error).with_context(|| format!("Cannot stage previous installation of '{name}'"));
    }
    if let Err(error) = fs::rename(&stage, &target) {
        let restore = fs::rename(&backup, &target);
        if let Err(restore_error) = restore {
            bail!("Cannot activate plugin update: {error}; restoring previous installation failed: {restore_error}");
        }
        fs::remove_dir_all(&stage).context("Cannot clean failed plugin update")?;
        return Err(error.into());
    }
    Ok(backup)
}

fn restore_replaced_installation(
    root: &Path, name: &str, backup: &Path,
) -> Result<()> {
    let home = checked_installed_root(root, false)?;
    let target = home.join(name);
    let failed = home.join(format!(".failed-update-{}", Uuid::new_v4()));
    fs::rename(&target, &failed)
        .with_context(|| format!("Cannot stage failed update of '{name}'"))?;
    if let Err(error) = fs::rename(backup, &target) {
        let recovery = fs::rename(&failed, &target);
        if let Err(recovery_error) = recovery {
            bail!("Cannot restore previous plugin '{name}': {error}; current version recovery failed: {recovery_error}");
        }
        return Err(error.into());
    }
    fs::remove_dir_all(failed).context("Cannot clean rolled-back plugin update")?;
    Ok(())
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

/// dependencyResolver.ts::findReverseDependents: disabling or uninstalling
/// a plugin warns about other enabled plugins whose declared dependencies
/// will become unsatisfied. A bare dependency inherits its declaring
/// marketplace; inline plugins match bare names across marketplaces.
fn reverse_dependents(root: &Path, config: &Config, target: &str) -> (Vec<String>, Vec<String>) {
    let home = installed_root(root);
    let mut dependents = Vec::new();
    let mut warnings = Vec::new();
    let read_origin = |name: &str| -> Result<Option<String>> {
        let origin = home.join(name).join(".atlas-plugin-origin.json");
        let metadata = match fs::symlink_metadata(&origin) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            bail!("Invalid plugin marketplace identity");
        }
        let value: Value = serde_json::from_slice(&fs::read(origin)?)?;
        Ok(Some(value["pluginId"].as_str()
            .context("Plugin marketplace identity must be a string")?.to_owned()))
    };
    let target_marketplace = match read_origin(target) {
        Ok(origin) => origin.and_then(|id| id.rsplit_once('@').map(|(_, market)| market.to_owned())),
        Err(error) => {
            warnings.push(format!("Cannot inspect plugin '{target}' origin: {error:#}"));
            None
        }
    };
    for name in &config.enabled_plugins {
        if name == target || !valid_name(name) {
            continue;
        }
        let directory = home.join(name);
        match fs::symlink_metadata(&directory) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
            Ok(_) => {
                warnings.push(format!("Cannot inspect redirected plugin '{name}'"));
                continue;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                warnings.push(format!("Cannot inspect plugin '{name}': {error}"));
                continue;
            }
        }
        let manifest = [directory.join(".claude-plugin/plugin.json"), directory.join("plugin.json")]
            .into_iter().find(|path| path.exists());
        let Some(manifest) = manifest else { continue };
        let parsed = (|| -> Result<Value> {
            let metadata = fs::symlink_metadata(&manifest)?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                bail!("Plugin manifest is redirected or invalid");
            }
            Ok(serde_json::from_slice(&fs::read(manifest)?)?)
        })();
        let value = match parsed {
            Ok(value) => value,
            Err(error) => {
                warnings.push(format!("Cannot inspect dependencies of '{name}': {error:#}"));
                continue;
            }
        };
        let Some(dependencies) = value.get("dependencies") else { continue };
        let Some(dependencies) = dependencies.as_array() else {
            warnings.push(format!("Plugin '{name}' dependencies must be an array"));
            continue;
        };
        let declaring_marketplace = match read_origin(name) {
            Ok(origin) => origin.and_then(|id| id.rsplit_once('@').map(|(_, market)| market.to_owned())),
            Err(error) => {
                warnings.push(format!("Cannot inspect plugin '{name}' origin: {error:#}"));
                continue;
            }
        };
        let mut required = false;
        for dependency in dependencies {
            let (dependency_name, explicit_marketplace) = if let Some(raw) = dependency.as_str() {
                let normalized = raw.split("@^").next().unwrap_or(raw);
                match normalized.split_once('@') {
                    Some((plugin, marketplace)) => (plugin, Some(marketplace)),
                    None => (normalized, None),
                }
            } else if let Some(object) = dependency.as_object() {
                let Some(plugin) = object.get("name").and_then(Value::as_str) else {
                    warnings.push(format!("Plugin '{name}' has a dependency without a name"));
                    continue;
                };
                (plugin, object.get("marketplace").and_then(Value::as_str))
            } else {
                warnings.push(format!("Plugin '{name}' has an invalid dependency"));
                continue;
            };
            if dependency_name != target {
                continue;
            }
            let required_marketplace = explicit_marketplace.or(declaring_marketplace.as_deref());
            if required_marketplace.is_none()
                || required_marketplace == target_marketplace.as_deref()
            {
                required = true;
                break;
            }
        }
        if required {
            dependents.push(name.clone());
        }
    }
    dependents.sort();
    dependents.dedup();
    (dependents, warnings)
}


/// Undo only directories created by the current install attempt. Never
/// follow redirected plugin storage; report a rollback failure explicitly.
fn rollback_new_installations(root: &Path, names: &[String]) -> Result<()> {
    if names.is_empty() { return Ok(()); }
    let home = checked_installed_root(root, false)?;
    for name in names.iter().rev() {
        let target = home.join(name);
        let metadata = fs::symlink_metadata(&target)
            .with_context(|| format!("Cannot inspect installed plugin '{name}' during rollback"))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            bail!("Cannot safely roll back plugin '{name}'");
        }
        fs::remove_dir_all(&target)
            .with_context(|| format!("Cannot roll back installed plugin '{name}'"))?;
    }
    Ok(())
}

/** Resolve the same dependency-first installation order as the source
 * dependencyResolver.ts::resolveDependencyClosure. Every source and managed
 * policy is checked before the first plugin is installed. */
fn dependency_install_plan(
    root: &Path,
    source: &Path,
    root_origin: Option<&str>,
    enabled: &[String],
) -> Result<Vec<(String, PathBuf, Option<String>, bool)>> {
    use std::collections::HashSet;

    fn dependencies(directory: &Path) -> Result<Vec<String>> {
        let standard = directory.join(".claude-plugin/plugin.json");
        let legacy = directory.join("plugin.json");
        let path = if standard.exists() { Some(standard) }
            else if legacy.exists() { Some(legacy) } else { None };
        let Some(path) = path else { return Ok(Vec::new()); };
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() || !metadata.is_file()
            || !path.canonicalize()?.starts_with(directory.canonicalize()?) {
            bail!("Plugin dependency manifest is redirected or invalid");
        }
        let manifest: Value = serde_json::from_slice(&fs::read(path)?)?;
        match manifest.get("dependencies") {
            None | Some(Value::Null) => Ok(Vec::new()),
            Some(Value::Array(items)) => items.iter().map(|item| {
                let id = item.as_str().context("Plugin dependency must be a string")?;
                if id.is_empty() { bail!("Plugin dependency is empty"); }
                Ok(id.to_owned())
            }).collect(),
            _ => bail!("Plugin dependencies must be an array"),
        }
    }

    fn visit(
        root: &Path,
        name: &str,
        source: PathBuf,
        origin: Option<String>,
        root_marketplace: Option<&str>,
        allowed_cross_marketplaces: &[String],
        is_root: bool,
        enabled: &[String],
        visited: &mut HashSet<String>,
        stack: &mut Vec<String>,
        ordered: &mut Vec<(String, PathBuf, Option<String>, bool)>,
    ) -> Result<()> {
        if !valid_name(name) { bail!("Invalid dependency plugin name '{name}'"); }
        if stack.iter().any(|item| item == name) {
            bail!("Plugin dependency cycle: {} -> {name}", stack.join(" -> "));
        }
        if visited.contains(name) {
            if ordered.iter().any(|(existing_name, _, existing_origin, _):
                &(String, PathBuf, Option<String>, bool)|
                existing_name == name && existing_origin != &origin) {
                bail!("Dependency '{name}' resolves to conflicting marketplaces");
            }
            return Ok(());
        }
        let target = checked_installed_root(root, false)?.join(name);
        let existing = match fs::symlink_metadata(&target) {
            Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => true,
            Ok(_) => bail!("Plugin '{name}' installation is redirected or invalid"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(error) => return Err(error.into()),
        };
        if existing {
            let existing_origin = installed_origin_id(&target)?;
            if origin.is_some() && existing_origin != origin {
                bail!("Installed plugin '{name}' belongs to a different marketplace");
            }
        }
        if crate::plugins::plugin_blocked_by_policy(root, name)? {
            bail!("Plugin '{name}' is disabled by managed policy");
        }
        if let Some(id) = origin.as_deref() {
            if crate::plugins::plugin_blocked_by_policy(root, id)? {
                bail!("Plugin '{id}' is disabled by managed policy");
            }
        }
        // The original resolver does not traverse dependencies already enabled
        // in the current scope, but the installed identity must be real.
        if !is_root && existing && enabled.iter().any(|item| item == name) {
            visited.insert(name.to_owned());
            return Ok(());
        }
        stack.push(name.to_owned());
        let dependency_source = if existing && !is_root { target.clone() } else { source.clone() };
        let mut required = dependencies(&dependency_source)?;
        if let Some((plugin, marketplace)) = origin.as_deref()
            .and_then(|id| id.split_once('@')) {
            for dependency in crate::marketplace_registry::local_plugin_dependencies(
                root, marketplace, plugin,
            )? {
                if !required.contains(&dependency) {
                    required.push(dependency);
                }
            }
        }
        for raw in required {
            let (dep_name, dep_marketplace) = match raw.split_once('@') {
                Some((plugin, market)) if !plugin.is_empty() && !market.is_empty() =>
                    (plugin, Some(market)),
                Some(_) => bail!("Invalid plugin dependency '{raw}'"),
                None => (raw.as_str(), origin.as_deref().and_then(|id| id.split_once('@').map(|(_, market)| market))),
            };
            if !valid_name(dep_name) { bail!("Invalid plugin dependency '{raw}'"); }
            let dep_origin = dep_marketplace.map(|market| format!("{dep_name}@{market}"));
            // Trust is inherited only from the root marketplace. An already
            // installed and enabled cross-market dependency is allowed, but
            // automatic cross-market installation requires explicit trust.
            if dep_marketplace != root_marketplace {
                let installed = checked_installed_root(root, false)?.join(dep_name);
                let already_enabled = enabled.iter().any(|item| item == dep_name)
                    && installed.is_dir();
                let explicitly_trusted = dep_marketplace.is_some_and(|market|
                    allowed_cross_marketplaces.iter().any(|trusted| trusted == market));
                if !already_enabled && !explicitly_trusted {
                    bail!("Cross-marketplace dependency '{raw}' is not trusted by the root marketplace");
                }
            }
            let dep_source = if let Some(market) = dep_marketplace {
                let installed = checked_installed_root(root, false)?.join(dep_name);
                if installed.is_dir() { installed }
                else { crate::marketplace_registry::resolve_local_plugin(root, market, dep_name)? }
            } else {
                let installed = checked_installed_root(root, false)?.join(dep_name);
                if installed.is_dir() { installed }
                else { bail!("Dependency '{raw}' requires a registered marketplace"); }
            };
            visit(root, dep_name, dep_source, dep_origin, root_marketplace,
                allowed_cross_marketplaces, false, enabled, visited, stack, ordered)?;
        }
        stack.pop();
        visited.insert(name.to_owned());
        ordered.push((name.to_owned(), source, origin, existing));
        Ok(())
    }

    let name = manifest_name(source)?;
    let root_marketplace = root_origin.and_then(|id|
        id.split_once('@').map(|(_, market)| market));
    let allowed_cross_marketplaces = match root_marketplace {
        Some(marketplace) => crate::marketplace_registry::local_marketplace_dependency_allowlist(
            root, marketplace,
        )?,
        None => Vec::new(),
    };
    let mut ordered = Vec::new();
    let mut visited = HashSet::new();
    visit(root, &name, source.to_owned(), root_origin.map(str::to_owned),
        root_marketplace, &allowed_cross_marketplaces,
        true, enabled, &mut visited, &mut Vec::new(), &mut ordered)?;
    Ok(ordered)
}

pub fn execute(root: &Path, command: &PluginCommand) -> Result<Value> {
    let root = root.canonicalize()?;
    // Lock before Config::load for mutations: concurrent plugin commands
    // must not overwrite each other's enabled-plugin settings. Read-only
    // commands preserve their no-write behavior (including no lock file).
    let mutating = match command {
        PluginCommand::Menu | PluginCommand::Manage | PluginCommand::Help
        | PluginCommand::Validate { .. } => false,
        PluginCommand::Marketplace { action, .. }
            if action.as_deref().unwrap_or("list") == "list" => false,
        _ => true,
    };
    let _lock = if mutating { Some(plugin_operation_lock()?) } else { None };
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
            let mut marketplace_id = None;
            let local = if let Some(plugin_name) = plugin {
                if let Some(marketplace) = marketplace.as_deref() {
                    marketplace_id = Some(format!("{plugin_name}@{marketplace}"));
                    crate::marketplace_registry::resolve_local_plugin(
                        &root, marketplace, plugin_name,
                    )?
                } else {
                    let (marketplace, path) =
                        crate::marketplace_registry::resolve_local_plugin_by_name(
                            &root, plugin_name,
                        )?;
                    marketplace_id = Some(format!("{plugin_name}@{marketplace}"));
                    path
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
            let policy = crate::permission_settings::SettingsPaths::for_workspace(&root)?;
            let actual_name = manifest_name(&local.canonicalize()?)?;
            if policy.plugin_blocked_by_policy(&actual_name)? {
                bail!("Plugin '{actual_name}' is disabled by managed policy");
            }
            if let Some(id) = marketplace_id.as_deref() {
                if policy.plugin_blocked_by_policy(id)? {
                    bail!("Plugin '{id}' is disabled by managed policy");
                }
            }
            let plan = dependency_install_plan(
                &root, &local, marketplace_id.as_deref(), &config.enabled_plugins,
            )?;
            let mut newly_installed = Vec::<String>::new();
            let mut replaced_root: Option<PathBuf> = None;
            let mut installed_names = Vec::<String>::new();
            for (name, location, origin, already_installed) in &plan {
                let operation = if *already_installed && name == &actual_name {
                    replace_local_with_origin(&root, location, origin.as_deref())
                        .map(|backup| replaced_root = Some(backup))
                } else if !already_installed {
                    install_local_with_origin(&root, location, origin.as_deref())
                        .map(|_| newly_installed.push(name.clone()))
                } else {
                    Ok(())
                };
                if let Err(error) = operation {
                    if let Err(cleanup) = rollback_new_installations(&root, &newly_installed) {
                        bail!("Plugin installation failed: {error:#}; rollback also failed: {cleanup:#}");
                    }
                    return Err(error);
                }
                installed_names.push(name.clone());
            }
            for name in &installed_names {
                if !config.enabled_plugins.contains(name) {
                    config.enabled_plugins.push(name.clone());
                }
            }
            if let Err(error) = config.save() {
                let restore = replaced_root.as_ref().map(|backup|
                    restore_replaced_installation(&root, &actual_name, backup));
                let cleanup = rollback_new_installations(&root, &newly_installed);
                if let Some(Err(restore_error)) = restore {
                    bail!("Cannot save plugin settings: {error:#}; previous installation restore failed: {restore_error:#}");
                }
                if let Err(cleanup_error) = cleanup {
                    bail!("Cannot save plugin settings: {error:#}; dependency rollback failed: {cleanup_error:#}");
                }
                return Err(error);
            }
            let mut warnings = Vec::<String>::new();
            if let Some(backup) = replaced_root {
                if let Err(error) = fs::remove_dir_all(&backup) {
                    warnings.push(format!("Previous plugin version could not be cleaned: {error}"));
                }
            }
            // Installation can succeed while the runtime loader reports
            // broken hooks, missing components, or dependency demotion.
            // Surface those diagnostics instead of reporting a silent success.
            let (active, runtime_errors) =
                crate::plugins::active_plugin_names(&root, &config.enabled_plugins);
            for name in &installed_names {
                if !active.contains(name) {
                    warnings.push(format!(
                        "Plugin '{name}' is installed and enabled but is not currently runnable"
                    ));
                }
                let prefix = format!("Plugin '{name}' ");
                warnings.extend(runtime_errors.iter()
                    .filter(|error| error.starts_with(&prefix)).cloned());
            }
            warnings.sort();
            warnings.dedup();
            let root_active = active.contains(&actual_name);
            let dependencies = installed_names.into_iter()
                .filter(|name| name != &actual_name).collect::<Vec<_>>();
            Ok(json!({
                "installed":actual_name,
                "active":root_active,
                "dependencies":dependencies,
                "warnings":warnings
            }))
        }
        PluginCommand::Enable { plugin } => {
            let name = plugin.as_ref().context("Plugin name is required")?;
            if !valid_name(name) {
                bail!("Invalid plugin name");
            }
            if crate::plugins::plugin_blocked_by_policy(&root, name)? {
                bail!("Plugin '{name}' is disabled by managed policy");
            }
            // PluginSet::load intentionally isolates unavailable plugins and
            // returns diagnostics rather than Err, so it cannot by itself
            // establish that an installation exists. Verify the installed
            // identity before changing the user's enabled state.
            let installed = checked_installed_root(&root, false)?.join(name);
            let metadata = fs::symlink_metadata(&installed)
                .with_context(|| format!("Plugin '{name}' is not installed"))?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                bail!("Plugin '{name}' installation is invalid");
            }
            // Managed policy is keyed by the full marketplace identity in
            // the original pluginPolicy.ts. A short-name-only check could
            // re-enable a plugin explicitly blocked as name@marketplace.
            if let Some(origin) = installed_origin_id(&installed)? {
                if crate::plugins::plugin_blocked_by_policy(&root, &origin)? {
                    bail!("Plugin '{origin}' is disabled by managed policy");
                }
            }
            let manifest = [installed.join(".claude-plugin/plugin.json"), installed.join("plugin.json")]
                .into_iter().find(|path| path.exists());
            if let Some(path) = manifest {
                let metadata = fs::symlink_metadata(&path)?;
                if metadata.file_type().is_symlink() || !metadata.is_file() {
                    bail!("Plugin '{name}' manifest is redirected or invalid");
                }
                let value: Value = serde_json::from_slice(&fs::read(path)?)?;
                if value.get("name").and_then(Value::as_str) != Some(name.as_str()) {
                    bail!("Plugin '{name}' manifest identity does not match installation");
                }
            }
            PluginSet::load(&root, std::slice::from_ref(name))?;
            if config.enabled_plugins.contains(name) {
                bail!("Plugin '{name}' is already enabled");
            }
            config.enabled_plugins.push(name.clone());
            config.save()?;
            let (active, runtime_errors) =
                crate::plugins::active_plugin_names(&root, &config.enabled_plugins);
            let prefix = format!("Plugin '{name}' ");
            let mut warnings = runtime_errors.into_iter()
                .filter(|error| error.starts_with(&prefix)).collect::<Vec<_>>();
            let runnable = active.contains(name);
            if !runnable {
                warnings.push(format!(
                    "Plugin '{name}' is enabled in settings but cannot run in this session"
                ));
            }
            warnings.sort();
            warnings.dedup();
            Ok(json!({"enabled":name,"active":runnable,"warnings":warnings}))
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
            let (reverse_dependents, warnings) = reverse_dependents(&root, &config, name);
            config.enabled_plugins.retain(|value| value != name);
            config.save()?;
            Ok(json!({"disabled":name,"reverseDependents":reverse_dependents,"warnings":warnings}))
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
            let (reverse_dependents, dependency_warnings) =
                reverse_dependents(&root, &config, name);
            if delete_data.is_none() {
                if let Some(size) = persistent_plugin_data_size(&root, name)? {
                    return Ok(json!({
                        "confirmationRequired":true,
                        "plugin":name,
                        "dataBytes":size,
                        "choices":["--delete-data","--keep-data"],
                        "reverseDependents":reverse_dependents,
                        "warnings":dependency_warnings
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
            let mut warnings = dependency_warnings;
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
                Ok(json!({"uninstalled":name,"dataPreserved":preserved,"reverseDependents":reverse_dependents}))
            } else {
                Ok(json!({"uninstalled":name,"dataPreserved":preserved,"reverseDependents":reverse_dependents,"warnings":warnings}))
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
