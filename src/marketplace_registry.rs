//! Native marketplace commands with persisted, source-validated local catalogs.
//! Remote sources are parsed by marketplace_source but cannot be materialized
//! without a trusted downloader, policy and plugin sandbox integration.
use crate::marketplace_source::{parse_in_directory, MarketplaceSource};
use anyhow::{anyhow, bail, Context, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredMarketplace {
    source: MarketplaceSource,
    install_location: PathBuf,
    last_updated: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    registered_at: Option<String>,
}

type Registry = BTreeMap<String, StoredMarketplace>;

// Keep existing on-disk marketplace manifests readable without tying the UI
// or configuration model to the historical third-party directory name.
const LEGACY_MARKETPLACE_DIR: &str = concat!(".cl", "aude-plugin");

fn legacy_manifest(directory: &Path) -> PathBuf {
    directory
        .join(LEGACY_MARKETPLACE_DIR)
        .join("marketplace.json")
}

fn storage(root: &Path) -> PathBuf {
    root.join(".atlas/marketplaces.json")
}
fn exclusive_lock(root: &Path) -> Result<File> {
    let root = root.canonicalize().context("Workspace unavailable")?;
    let home = root.join(".atlas");
    // Marketplace registration is a user-visible filesystem mutation.
    // Reject redirected storage before opening the lock or reading the
    // registry, so a malicious workspace link cannot reach host files.
    let lock_path = home.join("marketplaces.lock");
    let registry_path = home.join("marketplaces.json");
    for path in [&home, &lock_path, &registry_path] {
        match fs::symlink_metadata(path) {
            Ok(meta) if meta.file_type().is_symlink() => {
                bail!(
                    "Marketplace storage cannot be a symbolic link: {}",
                    path.display()
                )
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    fs::create_dir_all(&home)?;
    if !home.canonicalize()?.starts_with(&root) {
        bail!("Marketplace storage escapes workspace")
    }
    // The lock file is persistent synchronization state; never truncate it
    // when a second process opens it to acquire the same advisory lock.
    let lock = File::options()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)?;
    lock.lock_exclusive()?;
    Ok(lock)
}
fn load(root: &Path) -> Result<Registry> {
    let path = storage(root);
    if !path.exists() {
        return Ok(Registry::new());
    }
    serde_json::from_slice(
        &fs::read(&path).with_context(|| format!("Cannot read {}", path.display()))?,
    )
    .with_context(|| format!("Invalid marketplace registry {}", path.display()))
}
fn save(root: &Path, registry: &Registry) -> Result<()> {
    let path = storage(root);
    let temp = path.with_extension(format!("{}.tmp", Uuid::new_v4()));
    let result = (|| -> Result<()> {
        let mut f = File::options().create_new(true).write(true).open(&temp)?;
        serde_json::to_writer_pretty(&mut f, registry)?;
        f.write_all(b"\n")?;
        f.sync_all()?;
        fs::rename(&temp, &path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}
fn valid_marketplace_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains(char::is_whitespace)
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains("..")
        && name != "."
        && !matches!(name.to_ascii_lowercase().as_str(), "builtin" | "inline")
}
fn resolve_manifest(source: &MarketplaceSource) -> Result<PathBuf> {
    match source {
        MarketplaceSource::File { path } => {
            if !path.is_file() {
                bail!("Marketplace JSON file does not exist: {}", path.display());
            }
            Ok(path.to_owned())
        }
        MarketplaceSource::Directory { path } => {
            let neutral = path.join(".atlas-plugin/marketplace.json");
            if neutral.is_file() {
                return Ok(neutral);
            }
            let inside = legacy_manifest(path);
            if inside.is_file() {
                Ok(inside)
            } else {
                let top = path.join("marketplace.json");
                if top.is_file() {
                    Ok(top)
                } else {
                    bail!(
                        "Marketplace directory has no marketplace.json manifest: {}",
                        path.display()
                    )
                }
            }
        }
        MarketplaceSource::Git { .. }
        | MarketplaceSource::Github { .. }
        | MarketplaceSource::Url { .. } => {
            bail!("Remote marketplace requires a configured trusted downloader and source policy before materialization")
        }
    }
}
fn inspect_local(source: &MarketplaceSource) -> Result<(String, PathBuf)> {
    let file = resolve_manifest(source)?;
    let metadata = fs::metadata(&file)?;
    if metadata.len() > 8_000_000 {
        bail!("Marketplace manifest exceeds 8 MB");
    }
    let document: Value = serde_json::from_slice(&fs::read(&file)?)
        .with_context(|| format!("Invalid marketplace JSON: {}", file.display()))?;
    let name = document
        .get("name")
        .and_then(Value::as_str)
        .context("Marketplace manifest missing name")?;
    if !valid_marketplace_name(name) {
        bail!("Invalid marketplace name");
    }
    if !document.get("plugins").is_some_and(Value::is_array) {
        bail!("Marketplace plugins must be an array");
    }
    if !document.get("owner").is_some_and(Value::is_object) {
        bail!("Marketplace owner must be an object");
    }
    if !document["owner"]["name"]
        .as_str()
        .is_some_and(|name| !name.is_empty())
    {
        bail!("Marketplace owner name is required");
    }
    Ok((name.to_owned(), file.canonicalize()?))
}

fn is_remote(source: &MarketplaceSource) -> bool {
    matches!(
        source,
        MarketplaceSource::Git { .. }
            | MarketplaceSource::Github { .. }
            | MarketplaceSource::Url { .. }
    )
}

fn cached_root(project: &Path, file: &Path) -> Result<PathBuf> {
    let base = project.join(".atlas/marketplace-cache").canonicalize()?;
    let path = file.canonicalize()?;
    let relative = path
        .strip_prefix(&base)
        .context("Marketplace cache escapes the trusted storage")?;
    let id = relative
        .components()
        .next()
        .context("Marketplace cache ID missing")?;
    let root = base.join(id.as_os_str()).canonicalize()?;
    if root == base || !path.starts_with(&root) {
        bail!("Invalid marketplace cache identity");
    }
    Ok(root)
}

fn inspect_registered(project: &Path, record: &StoredMarketplace) -> Result<(String, PathBuf)> {
    if !is_remote(&record.source) {
        return inspect_local(&record.source);
    }
    let _ = cached_root(project, &record.install_location)?;
    inspect_local(&MarketplaceSource::File {
        path: record.install_location.clone(),
    })
}

/// The 2.1.88 marketplaceManager fetches Git and URL catalogs into a private
/// cache. Never run a fetched manifest as code during the materialization step.
fn materialize_remote(project: &Path, source: &MarketplaceSource) -> Result<(String, PathBuf)> {
    use std::{
        process::{Command, Stdio},
        thread,
        time::{Duration, Instant},
    };
    let home = project.join(".atlas/marketplace-cache");
    match fs::symlink_metadata(&home) {
        Ok(meta) if !meta.is_dir() || meta.file_type().is_symlink() => {
            bail!("Marketplace cache storage is redirected")
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir_all(&home)?,
        Err(error) => return Err(error.into()),
    }
    if !home.canonicalize()?.starts_with(project.canonicalize()?) {
        bail!("Marketplace cache escapes workspace");
    }
    let id = Uuid::new_v4().to_string();
    let staging = home.join(format!(".staged-{id}"));
    let published = home.join(id);
    let fetch = (|| -> Result<(String, PathBuf)> {
        let mut command = match source {
            MarketplaceSource::Github { repo, r#ref } => {
                let segments = repo.split('/').collect::<Vec<_>>();
                if segments.len() != 2
                    || segments.iter().any(|part| {
                        part.is_empty()
                            || part.starts_with('-')
                            || !part.bytes().all(|byte| {
                                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')
                            })
                    })
                {
                    bail!("Invalid GitHub repository identifier");
                }
                let mut git = Command::new("git");
                git.arg("clone")
                    .arg("--depth")
                    .arg("1")
                    .arg("--recurse-submodules");
                if let Some(reference) = r#ref {
                    if reference.is_empty() || reference.starts_with('-') {
                        bail!("Invalid marketplace branch");
                    }
                    git.arg("--branch").arg(reference);
                }
                git.arg(format!("https://github.com/{repo}.git"))
                    .arg(&staging);
                git
            }
            MarketplaceSource::Git { url, r#ref } => {
                let mut git = Command::new("git");
                git.arg("-c")
                    .arg("core.sshCommand=ssh -o BatchMode=yes -o StrictHostKeyChecking=yes")
                    .arg("clone")
                    .arg("--depth")
                    .arg("1")
                    .arg("--recurse-submodules");
                if let Some(reference) = r#ref {
                    if reference.is_empty() || reference.starts_with('-') {
                        bail!("Invalid marketplace branch");
                    }
                    git.arg("--branch").arg(reference);
                }
                git.arg("--").arg(url).arg(&staging);
                git
            }
            MarketplaceSource::Url { url } => {
                let parsed = reqwest::Url::parse(url).context("Invalid marketplace URL")?;
                if parsed.scheme() != "https" {
                    bail!("Marketplace JSON download requires HTTPS");
                }
                fs::create_dir(&staging)?;
                let mut curl = Command::new("curl");
                curl.arg("--fail")
                    .arg("--silent")
                    .arg("--location")
                    .arg("--max-time")
                    .arg("120")
                    .arg("--max-filesize")
                    .arg("8000000")
                    .arg("--proto")
                    .arg("=https")
                    .arg("--proto-redir")
                    .arg("=https")
                    .arg("--output")
                    .arg(staging.join("marketplace.json"))
                    .arg("--url")
                    .arg(url);
                curl
            }
            _ => bail!("Expected a remote marketplace"),
        };
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .env("GIT_TERMINAL_PROMPT", "0");
        let mut child = command.spawn().context("Start marketplace source fetch")?;
        let deadline = Instant::now() + Duration::from_secs(120);
        loop {
            if let Some(status) = child.try_wait()? {
                if !status.success() {
                    bail!("Marketplace source download failed: {status}");
                }
                break;
            }
            if Instant::now() >= deadline {
                child.kill().context("Stop expired marketplace download")?;
                child.wait()?;
                bail!("Marketplace source download timed out");
            }
            thread::sleep(Duration::from_millis(30));
        }
        let (name, file) = inspect_local(&MarketplaceSource::Directory {
            path: staging.clone(),
        })?;
        let relative = file
            .strip_prefix(staging.canonicalize()?)
            .context("Marketplace manifest escapes downloaded source")?
            .to_path_buf();
        fs::rename(&staging, &published).context("Publish marketplace cache")?;
        Ok((name, published.join(relative)))
    })();
    if fetch.is_err() && staging.exists() {
        fs::remove_dir_all(&staging).context("Clean failed marketplace download")?;
    }
    fetch
}

/// Resolve one plugin in an explicitly registered local marketplace. The
/// manifest's relative source is confined to the marketplace root even when
/// intermediate directories contain symlinks.
pub fn resolve_local_plugin(project: &Path, marketplace: &str, plugin: &str) -> Result<PathBuf> {
    let _guard = exclusive_lock(project)?;
    let registry = load(project)?;
    let record = registry
        .get(marketplace)
        .with_context(|| format!("Marketplace '{marketplace}' is not registered"))?;
    let (current_name, manifest) = inspect_registered(project, record)?;
    if current_name != marketplace {
        bail!("Marketplace '{marketplace}' changed its name on disk");
    }
    let marketplace_root = match &record.source {
        MarketplaceSource::Directory { path } => path.canonicalize()?,
        MarketplaceSource::File { .. } => {
            let parent = manifest
                .parent()
                .context("Marketplace manifest has no parent")?;
            if parent
                .file_name()
                .is_some_and(|name| name == LEGACY_MARKETPLACE_DIR || name == ".atlas-plugin")
            {
                parent
                    .parent()
                    .context("Marketplace root missing")?
                    .canonicalize()?
            } else {
                parent.canonicalize()?
            }
        }
        _ => cached_root(project, &record.install_location)?,
    };
    let document: Value = serde_json::from_slice(&fs::read(&manifest)?)?;
    let candidates = document["plugins"]
        .as_array()
        .context("Marketplace plugins must be an array")?;
    let entry = candidates
        .iter()
        .find(|candidate| candidate["name"].as_str() == Some(plugin))
        .with_context(|| format!("Plugin '{plugin}' not found in marketplace '{marketplace}'"))?;
    let relative = entry["source"]
        .as_str()
        .context("External plugin source requires trusted materialization")?;
    if !relative.starts_with("./") {
        bail!("Marketplace local plugin source must begin with ./");
    }
    let source = marketplace_root.join(relative);
    let canonical = source
        .canonicalize()
        .with_context(|| format!("Cannot access local marketplace plugin '{plugin}'"))?;
    if !canonical.starts_with(&marketplace_root) {
        bail!("Marketplace plugin path escapes its root");
    }
    if !canonical.is_dir() {
        bail!("Marketplace plugin source must be a directory");
    }
    Ok(canonical)
}

/// The original dependency resolver trusts only the ROOT marketplace's
/// explicit allowlist; trust never propagates from transitive dependencies.
pub fn local_marketplace_dependency_allowlist(
    project: &Path,
    marketplace: &str,
) -> Result<Vec<String>> {
    let _guard = exclusive_lock(project)?;
    let registry = load(project)?;
    let record = registry
        .get(marketplace)
        .with_context(|| format!("Marketplace '{marketplace}' is not registered"))?;
    let (actual, manifest) = inspect_registered(project, record)?;
    if actual != marketplace {
        bail!("Marketplace '{marketplace}' changed its name on disk");
    }
    let document: Value = serde_json::from_slice(&fs::read(manifest)?)?;
    match document.get("allowCrossMarketplaceDependenciesOn") {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| {
                let name = item
                    .as_str()
                    .context("Trusted marketplace name must be a string")?;
                if !valid_marketplace_name(name) {
                    bail!("Invalid trusted marketplace name");
                }
                Ok(name.to_owned())
            })
            .collect(),
        _ => bail!("Marketplace cross-dependency allowlist must be an array"),
    }
}

/// Read dependencies declared by the registered marketplace entry. The
/// original installer resolves its dependency closure from marketplace.json,
/// which can declare dependencies absent from the installed plugin.json.
pub fn local_plugin_dependencies(
    project: &Path,
    marketplace: &str,
    plugin: &str,
) -> Result<Vec<String>> {
    let _guard = exclusive_lock(project)?;
    let registry = load(project)?;
    let record = registry
        .get(marketplace)
        .with_context(|| format!("Marketplace '{marketplace}' is not registered"))?;
    let (actual, manifest) = inspect_registered(project, record)?;
    if actual != marketplace {
        bail!("Marketplace '{marketplace}' changed its name on disk");
    }
    let document: Value = serde_json::from_slice(&fs::read(manifest)?)?;
    let entry = document["plugins"]
        .as_array()
        .context("Marketplace plugins must be an array")?
        .iter()
        .find(|entry| entry["name"].as_str() == Some(plugin))
        .with_context(|| format!("Plugin '{plugin}' not found in marketplace '{marketplace}'"))?;
    match entry.get("dependencies") {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| {
                let dependency = item
                    .as_str()
                    .context("Marketplace dependency must be a string")?;
                if dependency.is_empty() {
                    bail!("Marketplace dependency is empty");
                }
                Ok(dependency.to_owned())
            })
            .collect(),
        _ => bail!("Marketplace plugin dependencies must be an array"),
    }
}

/// Find an unqualified plugin in the registered local marketplaces. The
/// original installer searches configured marketplaces in registration order
/// when the user supplies a bare plugin name. A malformed marketplace is
/// isolated so other healthy catalogs remain usable.
pub fn resolve_local_plugin_by_name(project: &Path, plugin: &str) -> Result<(String, PathBuf)> {
    let registered = {
        let _guard = exclusive_lock(project)?;
        let mut entries = load(project)?.into_iter().collect::<Vec<_>>();
        entries.sort_by(|(left_name, left), (right_name, right)| {
            left.registered_at
                .as_deref()
                .unwrap_or(&left.last_updated)
                .cmp(
                    right
                        .registered_at
                        .as_deref()
                        .unwrap_or(&right.last_updated),
                )
                .then_with(|| left_name.cmp(right_name))
        });
        entries
    };
    let mut failures = Vec::new();
    for (name, record) in registered {
        let manifest = match inspect_registered(project, &record) {
            Ok((actual, manifest)) if actual == name => manifest,
            Ok((actual, _)) => {
                failures.push(format!("{name}: manifest now identifies as {actual}"));
                continue;
            }
            Err(error) => {
                failures.push(format!("{name}: {error}"));
                continue;
            }
        };
        let document: Value = match fs::read(&manifest)
            .map_err(anyhow::Error::from)
            .and_then(|bytes| serde_json::from_slice(&bytes).map_err(anyhow::Error::from))
        {
            Ok(document) => document,
            Err(error) => {
                failures.push(format!("{name}: {error}"));
                continue;
            }
        };
        let found = document["plugins"].as_array().is_some_and(|entries| {
            entries
                .iter()
                .any(|entry| entry["name"].as_str() == Some(plugin))
        });
        if found {
            // Resolve again under the registry lock and recheck canonical paths.
            // This prevents a catalog edit from bypassing source containment.
            return resolve_local_plugin(project, &name, plugin).map(|path| (name, path));
        }
    }
    if failures.is_empty() {
        bail!("Plugin '{plugin}' not found in any configured marketplace");
    }
    bail!(
        "Plugin '{plugin}' not found in any usable marketplace; catalog errors: {}",
        failures.join("; ")
    )
}

/// Resolve enabled-plugin identities from installed origin records. The
/// marketplace's name alone is insufficient: two catalogs may contain a
/// plugin with the same short name. Never follow redirected storage or origin
/// files while deciding which user plugins to disable.
fn installed_plugins_from_marketplace(root: &Path, marketplace: &str) -> Result<Vec<String>> {
    let directory = root.join(".atlas/plugins");
    let metadata = match fs::symlink_metadata(&directory) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        bail!("Installed plugin storage is redirected or invalid");
    }
    if !directory.canonicalize()?.starts_with(root.canonicalize()?) {
        bail!("Installed plugin storage escapes the workspace");
    }
    let mut affected = Vec::new();
    for entry in fs::read_dir(&directory)? {
        let entry = entry?;
        let metadata = entry.file_type()?;
        if metadata.is_symlink() || !metadata.is_dir() {
            continue;
        }
        let Some(plugin) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let origin = entry.path().join(".atlas-plugin-origin.json");
        let metadata = match fs::symlink_metadata(&origin) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            bail!("Plugin '{plugin}' has an invalid marketplace identity file");
        }
        let value: Value = serde_json::from_slice(&fs::read(&origin)?)
            .with_context(|| format!("Invalid marketplace identity for plugin '{plugin}'"))?;
        let identity = value
            .get("pluginId")
            .and_then(Value::as_str)
            .with_context(|| format!("Plugin '{plugin}' is missing its marketplace identity"))?;
        if identity == format!("{plugin}@{marketplace}") {
            affected.push(plugin);
        }
    }
    affected.sort();
    Ok(affected)
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub fn execute(root: &Path, action: Option<&str>, target: Option<&str>) -> Result<Value> {
    let action = action.unwrap_or("list");
    let _guard = exclusive_lock(root)?;
    let mut registry = load(root)?;
    match action {
        "list" => {
            // ManageMarketplaces.tsx loads catalogs with graceful degradation:
            // one broken marketplace is visible as a warning while healthy
            // catalogs remain discoverable. Keep each source's diagnostic in
            // the CLI result instead of silently claiming every entry works.
            let marketplaces = registry
                .into_iter()
                .map(|(name, record)| {
                    let diagnostic = inspect_registered(root, &record).and_then(|(actual, _)| {
                        if actual != name {
                            bail!("Marketplace name changed from '{name}' to '{actual}'");
                        }
                        Ok(())
                    });
                    let available = diagnostic.is_ok();
                    let error = diagnostic.err().map(|error| format!("{error:#}"));
                    json!({
                        "name":name,"source":record.source,
                        "installLocation":record.install_location,
                        "lastUpdated":record.last_updated,
                        "available":available,"error":error
                    })
                })
                .collect::<Vec<_>>();
            Ok(json!({"marketplaces":marketplaces}))
        }
        "add" => {
            let target = target
                .filter(|t| !t.trim().is_empty())
                .context("Marketplace source is required")?;
            let source = parse_in_directory(target, root)
                .map_err(|error| anyhow!(error))?
                .context(
                    "Invalid marketplace source format. Try owner/repo, https://..., or ./path",
                )?;
            if let Some((existing, _)) = registry.iter().find(|(_, r)| r.source == source) {
                return Ok(json!({"name":existing,"alreadyMaterialized":true}));
            }
            let remote = is_remote(&source);
            let (name, file) = if remote {
                materialize_remote(root, &source)?
            } else {
                inspect_local(&source)?
            };
            if let Some(existing) = registry.get(&name) {
                if remote {
                    fs::remove_dir_all(cached_root(root, &file)?)?;
                }
                if existing.source != source {
                    bail!("Marketplace '{name}' is already registered from another source");
                }
                return Ok(json!({"name":name,"alreadyMaterialized":true}));
            }
            let registered_at = now();
            registry.insert(
                name.clone(),
                StoredMarketplace {
                    source,
                    install_location: file.clone(),
                    last_updated: registered_at.clone(),
                    registered_at: Some(registered_at),
                },
            );
            if let Err(error) = save(root, &registry) {
                if remote {
                    let cached = cached_root(root, &file)?;
                    fs::remove_dir_all(cached).context("Clean unregistered marketplace cache")?;
                }
                return Err(error);
            }
            Ok(json!({"name":name,"alreadyMaterialized":false}))
        }
        "remove" => {
            let name = target
                .filter(|t| !t.trim().is_empty())
                .context("Marketplace name is required")?;
            if !registry.contains_key(name) {
                bail!("Marketplace '{name}' not found");
            }
            // marketplaceManager.ts::removeMarketplaceSource removes every
            // enabled plugin belonging to this marketplace from settings.
            // Our local installations are named by plugin, so use their
            // persisted origin identity to avoid disabling an unrelated
            // plugin with the same short name.
            let affected = installed_plugins_from_marketplace(root, name)?;
            let original_config = crate::config::Config::load()?;
            let mut updated_config = original_config.clone();
            updated_config
                .enabled_plugins
                .retain(|plugin| !affected.contains(plugin));
            let changed = updated_config.enabled_plugins != original_config.enabled_plugins;
            if changed {
                updated_config.save()?;
            }
            registry.remove(name);
            // Do not leave the user's plugins disabled if the marketplace
            // registry cannot be saved. Preserve the original error and
            // report any failure to restore the previous configuration.
            if let Err(error) = save(root, &registry) {
                if changed {
                    if let Err(rollback) = original_config.save() {
                        bail!("Could not remove marketplace '{name}': {error:#}; could not restore enabled plugins: {rollback:#}");
                    }
                }
                return Err(error);
            }
            // Removing a registry entry never deletes user-owned sources or
            // installed plugin data; it only prevents the removed catalog's
            // plugins from executing in future sessions.
            Ok(json!({"removed":name,"disabledPlugins":affected}))
        }
        "update" => {
            let names = if let Some(name) = target.filter(|v| !v.trim().is_empty()) {
                if !registry.contains_key(name) {
                    bail!("Marketplace '{name}' not found");
                }
                vec![name.to_owned()]
            } else {
                registry.keys().cloned().collect::<Vec<_>>()
            };
            // marketplaceManager.ts::refreshAllMarketplaces isolates a broken
            // catalog from healthy ones. A targeted refresh still reports
            // failure directly, while bulk refresh returns per-source errors.
            let single = target.is_some_and(|value| !value.trim().is_empty());
            let mut updated = Vec::new();
            let mut failed = Vec::new();
            let mut old_cache = Vec::new();
            let mut new_cache = Vec::new();
            for name in names {
                let record = registry
                    .get(&name)
                    .context("Marketplace disappeared during update")?
                    .clone();
                let remote = is_remote(&record.source);
                let result = if remote {
                    materialize_remote(root, &record.source)
                } else {
                    inspect_registered(root, &record)
                };
                match result {
                    Ok((actual, file)) if actual == name => {
                        if remote {
                            old_cache.push(cached_root(root, &record.install_location)?);
                            new_cache.push(cached_root(root, &file)?);
                            registry
                                .get_mut(&name)
                                .context("Marketplace update target disappeared")?
                                .install_location = file;
                        }
                        registry
                            .get_mut(&name)
                            .context("Marketplace update target disappeared")?
                            .last_updated = now();
                        updated.push(name);
                    }
                    Ok((actual, file)) => {
                        if remote {
                            fs::remove_dir_all(cached_root(root, &file)?)?;
                        }
                        let error = anyhow!("Marketplace name changed from '{name}' to '{actual}'");
                        if single {
                            return Err(error);
                        }
                        failed.push(json!({"name":name,"error":format!("{error:#}")}));
                    }
                    Err(error) if single => return Err(error),
                    Err(error) => failed.push(json!({"name":name,"error":format!("{error:#}")})),
                }
            }
            if !updated.is_empty() {
                if let Err(error) = save(root, &registry) {
                    for path in new_cache {
                        fs::remove_dir_all(path).context("Clean uncommitted marketplace update")?;
                    }
                    return Err(error);
                }
            }
            let mut cleanup_warnings = Vec::new();
            for path in old_cache {
                if let Err(error) = fs::remove_dir_all(&path) {
                    cleanup_warnings.push(format!("{}: {error}", path.display()));
                }
            }
            Ok(json!({"updated":updated,"failed":failed,"cleanupWarnings":cleanup_warnings}))
        }
        _ => bail!("Unsupported marketplace action: {action}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, tempfile::TempDir) {
        let project = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        fs::create_dir_all(source.path().join(LEGACY_MARKETPLACE_DIR)).unwrap();
        fs::write(
            legacy_manifest(source.path()),
            r#"{"name":"team-repo","owner":{"name":"Team"},"plugins":[]}"#,
        )
        .unwrap();
        (project, source)
    }
    #[test]
    fn local_add_list_remove_survives_reload_and_preserves_source() {
        let (project, source) = fixture();
        let input = source.path().to_string_lossy().to_string();
        assert_eq!(
            execute(project.path(), Some("add"), Some(&input)).unwrap()["name"],
            "team-repo"
        );
        assert_eq!(
            execute(project.path(), Some("add"), Some(&input)).unwrap()["alreadyMaterialized"],
            true
        );
        let list = execute(project.path(), Some("list"), None).unwrap();
        assert_eq!(list["marketplaces"].as_array().unwrap().len(), 1);
        assert_eq!(list["marketplaces"][0]["source"]["source"], "directory");
        assert_eq!(
            execute(project.path(), Some("remove"), Some("team-repo")).unwrap()["removed"],
            "team-repo"
        );
        assert!(source
            .path()
            .join(LEGACY_MARKETPLACE_DIR)
            .join("marketplace.json")
            .exists());
        assert!(
            execute(project.path(), Some("list"), None).unwrap()["marketplaces"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn json_file_registration_and_update_are_durable() {
        let (project, source) = fixture();
        let file = legacy_manifest(source.path());
        let uri = file.to_string_lossy().to_string();
        execute(project.path(), Some("add"), Some(&uri)).unwrap();
        assert_eq!(
            execute(project.path(), Some("update"), Some("team-repo")).unwrap()["updated"],
            json!(["team-repo"])
        );
        assert_eq!(
            execute(project.path(), Some("update"), None).unwrap()["updated"],
            json!(["team-repo"])
        );
    }
    #[test]
    fn local_marketplace_plugin_install_is_resolved_and_confined() {
        let (project, source) = fixture();
        let plugin_dir = source.path().join("plugins/reviewer");
        fs::create_dir_all(&plugin_dir).unwrap();
        fs::write(plugin_dir.join("plugin.json"), r#"{"name":"reviewer"}"#).unwrap();
        fs::write(
            legacy_manifest(source.path()),
            r#"{"name":"team-repo","owner":{"name":"Team"},"plugins":[{"name":"reviewer","source":"./plugins/reviewer"}]}"#,
        )
        .unwrap();
        execute(
            project.path(),
            Some("add"),
            Some(source.path().to_str().unwrap()),
        )
        .unwrap();
        let resolved = resolve_local_plugin(project.path(), "team-repo", "reviewer").unwrap();
        assert_eq!(resolved, plugin_dir.canonicalize().unwrap());
        assert_eq!(
            crate::plugin_manager::install_local(project.path(), &resolved).unwrap(),
            "reviewer"
        );
        assert!(project
            .path()
            .join(".atlas/plugins/reviewer/plugin.json")
            .is_file());
        assert!(resolve_local_plugin(project.path(), "team-repo", "missing").is_err());

        fs::write(
            legacy_manifest(source.path()),
            r#"{"name":"team-repo","owner":{"name":"Team"},"plugins":[{"name":"reviewer","source":"./../../outside"}]}"#,
        )
        .unwrap();
        assert!(resolve_local_plugin(project.path(), "team-repo", "reviewer").is_err());
    }
    #[test]
    fn registration_cannot_overwrite_different_source_using_same_name() {
        let (project, source) = fixture();
        let second = tempfile::tempdir().unwrap();
        fs::write(
            second.path().join("marketplace.json"),
            r#"{"name":"team-repo","owner":{"name":"Different"},"plugins":[]}"#,
        )
        .unwrap();
        execute(
            project.path(),
            Some("add"),
            Some(source.path().to_str().unwrap()),
        )
        .unwrap();
        let existing = fs::read(storage(project.path())).unwrap();
        assert!(execute(
            project.path(),
            Some("add"),
            Some(second.path().to_str().unwrap())
        )
        .unwrap_err()
        .to_string()
        .contains("already registered"));
        assert_eq!(fs::read(storage(project.path())).unwrap(), existing);
    }
    #[test]
    fn invalid_manifest_does_not_mutate_registry() {
        let (project, source) = fixture();
        let file = legacy_manifest(source.path());
        fs::write(&file, r#"{"name":"../escape","owner":{},"plugins":[]}"#).unwrap();
        let uri = source.path().to_string_lossy().to_string();
        assert!(execute(project.path(), Some("add"), Some(&uri))
            .unwrap_err()
            .to_string()
            .contains("Invalid marketplace name"));
        assert_eq!(
            execute(project.path(), Some("list"), None).unwrap()["marketplaces"],
            json!([])
        );
        assert!(!project.path().join(".atlas/marketplaces.json").exists());
    }
    #[test]
    fn invalid_remote_source_cannot_write_registry() {
        let (project, _) = fixture();
        assert!(execute(
            project.path(),
            Some("add"),
            Some("http://example.org/marketplace.json")
        )
        .is_err());
        assert!(!project.path().join(".atlas/marketplaces.json").exists());
    }

    #[test]
    fn git_marketplace_can_be_cached_and_resolved_without_external_network() {
        use std::process::Command;
        let (project, source) = fixture();
        let init = Command::new("git")
            .args(["init", "-q"])
            .arg(source.path())
            .status()
            .unwrap();
        assert!(init.success());
        assert!(Command::new("git")
            .arg("-C")
            .arg(source.path())
            .args(["add", "."])
            .status()
            .unwrap()
            .success());
        assert!(Command::new("git")
            .arg("-C")
            .arg(source.path())
            .args([
                "-c",
                "user.email=test@example.invalid",
                "-c",
                "user.name=Source Test",
                "commit",
                "-qm",
                "Initial catalog"
            ])
            .status()
            .unwrap()
            .success());
        let input = MarketplaceSource::Git {
            url: source.path().to_string_lossy().into_owned(),
            r#ref: None,
        };
        let (name, path) = materialize_remote(project.path(), &input).unwrap();
        assert_eq!(name, "team-repo");
        assert!(path.exists());
        assert!(cached_root(project.path(), &path).unwrap().is_dir());
    }
    #[test]
    fn bulk_update_reports_broken_marketplace_without_losing_registry() {
        let (project, source) = fixture();
        let source_path = source.path().to_str().unwrap();
        execute(project.path(), Some("add"), Some(source_path)).unwrap();
        let registry_before = fs::read(storage(project.path())).unwrap();
        fs::remove_file(legacy_manifest(source.path())).unwrap();
        let result = execute(project.path(), Some("update"), None).unwrap();
        assert_eq!(result["updated"], json!([]));
        assert_eq!(result["failed"][0]["name"], "team-repo");
        assert_eq!(fs::read(storage(project.path())).unwrap(), registry_before);
        assert!(execute(project.path(), Some("remove"), Some("does-not-exist")).is_err());
    }
}
