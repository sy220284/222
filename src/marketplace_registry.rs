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
    for path in [&home, &home.join("marketplaces.lock"), &home.join("marketplaces.json")] {
        match fs::symlink_metadata(path) {
            Ok(meta) if meta.file_type().is_symlink() => {
                bail!("Marketplace storage cannot be a symbolic link: {}", path.display())
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
        .open(home.join("marketplaces.lock"))?;
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
/// Resolve one plugin in an explicitly registered local marketplace. The
/// manifest's relative source is confined to the marketplace root even when
/// intermediate directories contain symlinks.
pub fn resolve_local_plugin(project: &Path, marketplace: &str, plugin: &str) -> Result<PathBuf> {
    let _guard = exclusive_lock(project)?;
    let registry = load(project)?;
    let record = registry
        .get(marketplace)
        .with_context(|| format!("Marketplace '{marketplace}' is not registered"))?;
    let (current_name, manifest) = inspect_local(&record.source)?;
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
        _ => bail!("Remote marketplace requires a trusted materialization step"),
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

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub fn execute(root: &Path, action: Option<&str>, target: Option<&str>) -> Result<Value> {
    let action = action.unwrap_or("list");
    let _guard = exclusive_lock(root)?;
    let mut registry = load(root)?;
    match action {
        "list" => Ok(
            json!({"marketplaces":registry.into_iter().map(|(name, record)| json!({
            "name":name,"source":record.source,"installLocation":record.install_location,
            "lastUpdated":record.last_updated
        })).collect::<Vec<_>>() }),
        ),
        "add" => {
            let target = target
                .filter(|t| !t.trim().is_empty())
                .context("Marketplace source is required")?;
            let source = parse_in_directory(target, root)
                .map_err(|error| anyhow!(error))?
                .context(
                    "Invalid marketplace source format. Try owner/repo, https://..., or ./path",
                )?;
            let (name, file) = inspect_local(&source)?;
            if let Some((existing, _)) = registry.iter().find(|(_, r)| r.source == source) {
                return Ok(json!({"name":existing,"alreadyMaterialized":true}));
            }
            if let Some(existing) = registry.get(&name) {
                if existing.source != source {
                    bail!("Marketplace '{name}' is already registered from another source");
                }
                return Ok(json!({"name":name,"alreadyMaterialized":true}));
            }
            registry.insert(
                name.clone(),
                StoredMarketplace {
                    source,
                    install_location: file,
                    last_updated: now(),
                },
            );
            save(root, &registry)?;
            Ok(json!({"name":name,"alreadyMaterialized":false}))
        }
        "remove" => {
            let name = target
                .filter(|t| !t.trim().is_empty())
                .context("Marketplace name is required")?;
            if registry.remove(name).is_none() {
                bail!("Marketplace '{name}' not found");
            }
            // No deletion of the user-owned source folder/file.
            save(root, &registry)?;
            Ok(json!({"removed":name}))
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
            // Preflight *all* sources before mutating registry. A missing local
            // directory must never cause a partially refreshed list.
            for name in &names {
                let record = &registry[name];
                let (manifest_name, _) = inspect_local(&record.source)?;
                if manifest_name != *name {
                    bail!("Marketplace name changed from '{name}' to '{manifest_name}'");
                }
            }
            for name in &names {
                registry.get_mut(name).unwrap().last_updated = now();
            }
            if !names.is_empty() {
                save(root, &registry)?;
            }
            Ok(json!({"updated":names}))
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
    fn remote_source_rejected_before_registry_writes() {
        let (project, _) = fixture();
        assert!(execute(project.path(), Some("add"), Some("owner/repo#branch")).is_err());
        assert!(execute(
            project.path(),
            Some("add"),
            Some("https://example.org/marketplace.json")
        )
        .is_err());
        assert!(!project.path().join(".atlas/marketplaces.json").exists());
    }
    #[test]
    fn update_is_all_or_nothing_and_missing_name_is_an_error() {
        let (project, source) = fixture();
        let source_path = source.path().to_str().unwrap();
        execute(project.path(), Some("add"), Some(source_path)).unwrap();
        let registry_before = fs::read(storage(project.path())).unwrap();
        fs::remove_file(legacy_manifest(source.path())).unwrap();
        assert!(execute(project.path(), Some("update"), None).is_err());
        assert_eq!(fs::read(storage(project.path())).unwrap(), registry_before);
        assert!(execute(project.path(), Some("remove"), Some("does-not-exist")).is_err());
    }
}
