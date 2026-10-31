//! Locally persisted, explicitly invoked model-neutral task triggers.
//! No remote worker, background process, cloud schedule or implicit execution.
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use uuid::Uuid;

const MAX_TRIGGERS: usize = 128;
const MAX_PROMPT_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Trigger {
    pub prompt: String,
}

type Store = BTreeMap<String, Trigger>;

fn check_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > 64
        || !name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        bail!("Trigger name must be 1-64 ASCII letters, numbers, hyphens or underscores");
    }
    Ok(())
}

fn store_dir(root: &Path) -> PathBuf {
    root.join(".atlas").join("local-triggers")
}
fn store_path(root: &Path) -> PathBuf {
    store_dir(root).join("tasks.json")
}
fn legacy_path(root: &Path) -> PathBuf {
    root.join(".atlas").join("local_triggers.json")
}
fn ensure_private_directory(dir: &Path) -> Result<()> {
    // Workspaces may be arbitrary Git repositories. A gitignore in the 222
    // source tree cannot protect task prompts in a different repository.
    let path = dir.join(".gitignore");
    const RULES: &str = "*\n!.gitignore\n";
    let mut options = fs::OpenOptions::new();
    options.create_new(true).write(true);
    match options.open(&path) {
        Ok(mut file) => {
            file.write_all(RULES.as_bytes())?;
            file.sync_all()?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let present =
                fs::read_to_string(&path).context("Cannot verify local task privacy rules")?;
            if present != RULES {
                bail!(
                    "Local task store privacy rules have been modified: {}",
                    path.display()
                );
            }
        }
        Err(error) => return Err(error).context("Cannot create task privacy rules"),
    }
    Ok(())
}

/// Reject symlinked local task storage on both reads and writes. The task
/// directory belongs to the invoking workspace and holds private prompts;
/// an external symlink must never let get/list read another directory.
fn validate_storage_path(root: &Path, path: &Path) -> Result<()> {
    let workspace = root
        .canonicalize()
        .context("Cannot resolve task workspace")?;
    let relative = path
        .strip_prefix(root)
        .context("Task storage path is outside workspace")?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    bail!(
                        "Task storage path contains a symlink: {}",
                        current.display()
                    );
                }
                let actual = current.canonicalize().with_context(|| {
                    format!("Cannot verify task storage: {}", current.display())
                })?;
                if !actual.starts_with(&workspace) {
                    bail!("Task storage escapes workspace: {}", current.display());
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("Cannot inspect task storage: {}", current.display()))
            }
        }
    }
    Ok(())
}

fn load(root: &Path) -> Result<Store> {
    let primary = store_path(root);
    let legacy = legacy_path(root);
    validate_storage_path(root, &primary)?;
    validate_storage_path(root, &legacy)?;
    let path = if primary.exists() { primary } else { legacy };
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let meta = fs::metadata(&path).context("Cannot inspect trigger store")?;
    if meta.len() > (MAX_TRIGGERS * MAX_PROMPT_BYTES) as u64 {
        bail!("Local trigger store exceeds the accepted size limit");
    }
    let bytes = fs::read(&path).context("Cannot read trigger store")?;
    let entries: Store =
        serde_json::from_slice(&bytes).context("Local trigger store is corrupted")?;
    if entries.len() > MAX_TRIGGERS {
        bail!("Too many local triggers");
    }
    for (name, record) in &entries {
        check_name(name)?;
        if record.prompt.trim().is_empty() || record.prompt.len() > MAX_PROMPT_BYTES {
            bail!("Invalid stored trigger prompt for {name}");
        }
    }
    Ok(entries)
}

/// Release an OS advisory lock on every exit path, including an early
/// validation error or cancellation. File close alone is platform-dependent.
struct TriggerFileLock(fs::File);
impl Drop for TriggerFileLock {
    fn drop(&mut self) {
        if let Err(error) = fs2::FileExt::unlock(&self.0) {
            eprintln!("Cannot release local task store lock: {error}");
        }
    }
}

fn mutate_store(root: &Path, change: impl FnOnce(&mut Store) -> Result<()>) -> Result<()> {
    let dir = store_dir(root);
    validate_storage_path(root, &store_path(root))?;
    validate_storage_path(root, &legacy_path(root))?;
    fs::create_dir_all(&dir).context("Cannot create local trigger directory")?;
    let workspace = root
        .canonicalize()
        .context("Cannot resolve task workspace")?;
    let actual = dir
        .canonicalize()
        .context("Cannot resolve local task store")?;
    if !actual.starts_with(&workspace) {
        bail!("Local task store must remain inside the workspace");
    }
    let lock_path = dir.join("lock");
    let mut options = fs::OpenOptions::new();
    options.create(true).read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let lock_file = options
        .open(&lock_path)
        .context("Cannot open local task lock file")?;
    fs2::FileExt::try_lock_exclusive(&lock_file)
        .context("Another local trigger update is running")?;
    let _guard = TriggerFileLock(lock_file);
    let mut entries = load(root)?;
    change(&mut entries)?;
    ensure_private_directory(&dir)?;
    let path = store_path(root);
    let temp = dir.join(format!(".local_triggers-{}.tmp", Uuid::new_v4()));
    let result = (|| -> Result<()> {
        let bytes = serde_json::to_vec_pretty(&entries)?;
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temp)
            .context("Cannot create temporary trigger store")?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(&temp, &path).context("Cannot atomically replace trigger store")?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result?;
    // Only delete the legacy location after the durable new file exists.
    let old = legacy_path(root);
    if old.is_file() {
        fs::remove_file(&old).context("Cannot remove legacy trigger store after migration")?;
    }
    Ok(())
}

pub fn set(root: &Path, name: &str, prompt: &str) -> Result<()> {
    check_name(name)?;
    let prompt = prompt.trim();
    if prompt.is_empty() || prompt.len() > MAX_PROMPT_BYTES {
        bail!("Trigger prompt must be 1-65536 bytes");
    }
    mutate_store(root, |entries| {
        if !entries.contains_key(name) && entries.len() >= MAX_TRIGGERS {
            bail!("Local trigger limit reached");
        }
        entries.insert(
            name.to_owned(),
            Trigger {
                prompt: prompt.to_owned(),
            },
        );
        Ok(())
    })
}

/// Source-aligned local create: fail rather than overwrite an existing task.
pub fn create(root: &Path, name: &str, prompt: &str) -> Result<()> {
    check_name(name)?;
    let prompt = prompt.trim();
    if prompt.is_empty() || prompt.len() > MAX_PROMPT_BYTES {
        bail!("Trigger prompt must be 1-65536 bytes");
    }
    mutate_store(root, |entries| {
        if entries.contains_key(name) {
            bail!("Local trigger already exists: {name}");
        }
        if entries.len() >= MAX_TRIGGERS {
            bail!("Local trigger limit reached");
        }
        entries.insert(
            name.to_owned(),
            Trigger {
                prompt: prompt.to_owned(),
            },
        );
        Ok(())
    })
}

/// Source-aligned local update: only an existing task may be changed.
pub fn change(root: &Path, name: &str, prompt: &str) -> Result<()> {
    check_name(name)?;
    let prompt = prompt.trim();
    if prompt.is_empty() || prompt.len() > MAX_PROMPT_BYTES {
        bail!("Trigger prompt must be 1-65536 bytes");
    }
    mutate_store(root, |entries| {
        let existing = entries
            .get_mut(name)
            .with_context(|| format!("No local trigger named {name}"))?;
        existing.prompt = prompt.to_owned();
        Ok(())
    })
}

pub fn delete(root: &Path, name: &str) -> Result<()> {
    check_name(name)?;
    mutate_store(root, |entries| {
        if entries.remove(name).is_none() {
            bail!("No local trigger named {name}");
        }
        Ok(())
    })
}

pub fn get(root: &Path, name: &str) -> Result<String> {
    check_name(name)?;
    load(root)?
        .get(name)
        .map(|v| v.prompt.clone())
        .with_context(|| format!("No local trigger named {name}"))
}

pub fn list(root: &Path) -> Result<Vec<String>> {
    Ok(load(root)?.into_keys().collect())
}

/// Provider-neutral replacement for the original RemoteTrigger model tool.
/// No scheduling metadata, remote session, or spawned worker is accepted.
pub fn model_definition() -> serde_json::Value {
    serde_json::json!({"type":"function","function":{
        "name":"LocalTrigger",
        "description":"Manage explicit, user-authorized local reusable tasks. create accepts a prompt without trigger_id and generates one; get/update/run require trigger_id. run returns the saved instructions to be performed in the SAME current Agent turn. No cloud jobs or scheduled/remote execution.",
        "parameters":{"type":"object","properties":{
            "action":{"type":"string","enum":["list","get","create","update","run"]},
            "trigger_id":{"type":"string","description":"Required for get/update/run, optional for create"},
            "body":{"type":"object","properties":{"prompt":{"type":"string"}},
                "required":["prompt"],"additionalProperties":false}
        },"required":["action"],"additionalProperties":false}
    }})
}
pub fn validate_model_input(args: &serde_json::Value) -> Result<()> {
    let object = args
        .as_object()
        .context("LocalTrigger requires a JSON object")?;
    for key in object.keys() {
        if !matches!(key.as_str(), "action" | "trigger_id" | "body") {
            bail!("Unknown LocalTrigger field: {key}");
        }
    }
    let action = object
        .get("action")
        .and_then(serde_json::Value::as_str)
        .context("LocalTrigger requires action")?;
    match action {
        "list" => {
            if object.contains_key("body") || object.contains_key("trigger_id") {
                bail!("LocalTrigger list accepts only action");
            }
        }
        "get" | "run" => {
            check_name(
                object
                    .get("trigger_id")
                    .and_then(serde_json::Value::as_str)
                    .context("LocalTrigger requires trigger_id")?,
            )?;
            if object.contains_key("body") {
                bail!("LocalTrigger get/run cannot include body");
            }
        }
        "create" | "update" => {
            if action == "update" {
                check_name(
                    object
                        .get("trigger_id")
                        .and_then(serde_json::Value::as_str)
                        .context("LocalTrigger update requires trigger_id")?,
                )?;
            } else if object.contains_key("trigger_id") {
                check_name(
                    object
                        .get("trigger_id")
                        .and_then(serde_json::Value::as_str)
                        .context("LocalTrigger create trigger_id must be a string")?,
                )?;
            }
            let body = object
                .get("body")
                .and_then(serde_json::Value::as_object)
                .context("LocalTrigger requires a body object")?;
            if body.len() != 1 || !body.contains_key("prompt") {
                bail!("LocalTrigger accepts only body.prompt; scheduling metadata is unsupported");
            }
            let prompt = body
                .get("prompt")
                .and_then(serde_json::Value::as_str)
                .context("LocalTrigger body.prompt must be a string")?;
            if prompt.trim().is_empty() || prompt.len() > MAX_PROMPT_BYTES {
                bail!("LocalTrigger prompt must be 1-65536 bytes");
            }
        }
        _ => bail!("Unknown LocalTrigger action: {action}"),
    }
    Ok(())
}

/// Source UI.tsx renders the action and optional trigger id in the tool
/// header. Use the same text in local SDK tool events.
pub fn render_tool_use_message(args: &serde_json::Value) -> String {
    let action = args
        .get("action")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let name = args.get("trigger_id").and_then(serde_json::Value::as_str);
    format!(
        "{action}{}",
        name.map(|id| format!(" {id}")).unwrap_or_default()
    )
}

/// Source UI.tsx prints the status and the number of JSON body lines.
/// This summary is display metadata; the original tool result stays intact.
pub fn render_tool_result_message(result: &str) -> String {
    if let Some((status, json)) = result.split_once('\n') {
        if status.starts_with("HTTP ") {
            return format!("{status} ({} lines)", json.lines().count().max(1));
        }
    }
    "LocalTrigger failed".to_owned()
}

/// The remote API's observable tool result is status plus raw JSON. The
/// provider-neutral local equivalent keeps that wire format without making
/// any HTTP request, and returns the selected identifier on create.
fn model_result(status: u16, json: serde_json::Value) -> String {
    format!("HTTP {status}\n{json}")
}

/// A model tool cannot recursively launch an Agent. "run" makes the
/// authorized saved instructions available to the current model round, which
/// can execute them using its existing local tools and permission checks.
pub fn model_call(
    root: &Path,
    args: &serde_json::Value,
    mut authorize_mutation: impl FnMut() -> Result<()>,
) -> Result<String> {
    validate_model_input(args)?;
    let action = args["action"].as_str().context("Missing action")?;
    let name = args.get("trigger_id").and_then(serde_json::Value::as_str);
    match action {
        "list" => Ok(model_result(200, serde_json::json!(list(root)?))),
        "get" => {
            let name = name.context("Missing trigger_id")?;
            Ok(model_result(
                200,
                serde_json::json!({
                    "trigger_id":name, "prompt":get(root,name)?
                }),
            ))
        }
        "create" | "update" => {
            authorize_mutation()?;
            let generated;
            let name = if action == "create" {
                // Original inputSchema allows body-only create. The remote
                // server supplies an id; the local store must do the same.
                generated = Uuid::new_v4().to_string();
                name.unwrap_or(&generated)
            } else {
                name.context("Missing trigger_id")?
            };
            let prompt = args
                .pointer("/body/prompt")
                .and_then(serde_json::Value::as_str)
                .context("Missing prompt")?;
            if action == "create" {
                create(root, name, prompt)?;
            } else {
                change(root, name, prompt)?;
            }
            Ok(model_result(
                if action == "create" { 201 } else { 200 },
                serde_json::json!({
                    "trigger_id":name, "prompt":prompt,
                    "action":action, "storage":"local"
                }),
            ))
        }
        "run" => {
            authorize_mutation()?;
            let name = name.context("Missing trigger_id")?;
            let prompt = get(root, name)?;
            Ok(model_result(
                200,
                serde_json::json!({
                    "trigger_id":name,
                    "status":"accepted",
                    "execution":"current-agent-turn",
                    "prompt":prompt,
                    "instructions":"Execute the saved task using the current Agent's tools and permissions, and report its actual result. This request does not start a background or scheduled agent."
                }),
            ))
        }
        _ => bail!("Unsupported LocalTrigger action"),
    }
}

#[test]
fn model_tool_contract_rejects_schedules_and_invalid_actions() {
    let root = tempfile::tempdir().unwrap();
    let valid = serde_json::json!({"action":"create","trigger_id":"manual","body":{"prompt":"review local files"}});
    assert!(validate_model_input(&valid).is_ok());
    assert!(model_call(root.path(), &valid, || Ok(())).is_ok());
    assert!(model_call(
        root.path(),
        &serde_json::json!({"action":"get","trigger_id":"manual"}),
        || Ok(())
    )
    .unwrap()
    .contains("review local files"));
    let auto = model_call(
        root.path(),
        &serde_json::json!({"action":"create","body":{"prompt":"auto id task"}}),
        || Ok(()),
    )
    .unwrap();
    assert!(auto.starts_with("HTTP 201\\n".replace("\\n", "\n").as_str()));
    let auto_json: serde_json::Value =
        serde_json::from_str(auto.split_once('\n').unwrap().1).unwrap();
    let auto_id = auto_json["trigger_id"].as_str().unwrap();
    assert_eq!(get(root.path(), auto_id).unwrap(), "auto id task");
    assert!(auto_json["storage"] == "local");
    assert_eq!(
        render_tool_use_message(&serde_json::json!({
            "action":"run","trigger_id":"manual"
        })),
        "run manual"
    );
    assert_eq!(
        render_tool_result_message("HTTP 200\n{}"),
        "HTTP 200 (1 lines)"
    );
    assert_eq!(
        render_tool_result_message("ERROR: denied"),
        "LocalTrigger failed"
    );
    let task = model_call(
        root.path(),
        &serde_json::json!({"action":"run","trigger_id":"manual"}),
        || Ok(()),
    )
    .unwrap();
    assert!(task.contains("current-agent-turn"));
    assert!(task.starts_with("HTTP 200\n"));
    assert!(model_call(
        root.path(),
        &serde_json::json!({"action":"update","trigger_id":"manual","body":{"prompt":"mutate"}}),
        || anyhow::bail!("requires approval")
    )
    .is_err());
    assert_eq!(get(root.path(), "manual").unwrap(), "review local files");
    for args in [
        serde_json::json!({"action":"create","trigger_id":"future","body":{"prompt":"next","schedule":"hourly"}}),
        serde_json::json!({"action":"run","trigger_id":"manual","schedule":"hourly"}),
        serde_json::json!({"action":"delete","trigger_id":"manual"}),
        serde_json::json!({"action":"list","trigger_id":"manual"}),
    ] {
        assert!(validate_model_input(&args).is_err(), "{args}");
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn triggers_persist_run_same_prompt_and_delete() {
        let root = tempfile::tempdir().unwrap();
        set(root.path(), "review_ci", "检查 CI 失败").unwrap();
        set(root.path(), "review_ci", "使用已配置模型检查 Git 变更").unwrap();
        assert_eq!(list(root.path()).unwrap(), vec!["review_ci"]);
        assert!(get(root.path(), "review_ci")
            .unwrap()
            .contains("已配置模型"));
        delete(root.path(), "review_ci").unwrap();
        assert!(get(root.path(), "review_ci").is_err());
        assert!(list(root.path()).unwrap().is_empty());
    }
    #[test]
    fn create_get_update_respect_existing_record_preconditions() {
        let root = tempfile::tempdir().unwrap();
        assert!(change(root.path(), "missing", "new prompt").is_err());
        create(root.path(), "simple", "draft prompt").unwrap();
        assert!(create(root.path(), "simple", "overwrite").is_err());
        assert_eq!(get(root.path(), "simple").unwrap(), "draft prompt");
        change(root.path(), "simple", "revised prompt").unwrap();
        assert_eq!(get(root.path(), "simple").unwrap(), "revised prompt");
        assert_eq!(list(root.path()).unwrap(), vec!["simple"]);
    }
    #[test]
    fn stale_lock_file_does_not_block_subsequent_updates() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(store_dir(root.path())).unwrap();
        fs::write(store_dir(root.path()).join("lock"), b"stale").unwrap();
        set(root.path(), "recovered", "execute explicitly").unwrap();
        assert_eq!(get(root.path(), "recovered").unwrap(), "execute explicitly");
    }
    #[test]
    fn current_os_lock_rejects_concurrent_updates() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(store_dir(root.path())).unwrap();
        let held = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(store_dir(root.path()).join("lock"))
            .unwrap();
        fs2::FileExt::try_lock_exclusive(&held).unwrap();
        assert!(set(root.path(), "blocked", "prompt").is_err());
        fs2::FileExt::unlock(&held).unwrap();
        set(root.path(), "unblocked", "prompt").unwrap();
    }
    #[test]
    fn invalid_names_or_empty_tasks_do_not_write_store() {
        let root = tempfile::tempdir().unwrap();
        assert!(set(root.path(), "../outside", "message").is_err());
        assert!(set(root.path(), "valid", "  ").is_err());
        assert!(get(root.path(), "never").is_err());
        assert!(!store_path(root.path()).exists());
    }
    #[test]
    fn prompts_are_in_an_independently_gitignored_directory() {
        let root = tempfile::tempdir().unwrap();
        set(root.path(), "private", "sensitive-local-prompt").unwrap();
        let directory = store_dir(root.path());
        assert_eq!(
            fs::read_to_string(directory.join(".gitignore")).unwrap(),
            "*\\n!.gitignore\\n".replace("\\n", "\n")
        );
        assert!(directory.join("tasks.json").is_file());
        assert!(!legacy_path(root.path()).exists());
    }
    #[test]
    fn task_prompt_is_ignored_by_an_unconfigured_git_repository() {
        let root = tempfile::tempdir().unwrap();
        let git = std::process::Command::new("git")
            .arg("-C")
            .arg(root.path())
            .args(["init", "-q"])
            .status()
            .unwrap();
        assert!(git.success());
        set(root.path(), "private", "do not publish my task").unwrap();
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(root.path())
            .args(["status", "--porcelain", "--untracked-files=all"])
            .output()
            .unwrap();
        assert!(status.status.success());
        let visible = String::from_utf8(status.stdout).unwrap();
        assert!(!visible.contains("tasks.json"), "{visible}");
    }
    #[test]
    fn legacy_prompts_are_read_and_migrated_without_loss() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join(".atlas")).unwrap();
        fs::write(
            legacy_path(root.path()),
            r#"{"legacy":{"prompt":"saved before migration"}}"#,
        )
        .unwrap();
        assert_eq!(
            get(root.path(), "legacy").unwrap(),
            "saved before migration"
        );
        change(root.path(), "legacy", "updated after migration").unwrap();
        assert_eq!(
            get(root.path(), "legacy").unwrap(),
            "updated after migration"
        );
        assert!(!legacy_path(root.path()).exists());
        assert!(store_path(root.path()).exists());
    }
    #[cfg(unix)]
    #[test]
    fn neither_read_nor_write_follows_symlinked_local_task_directory() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join(".atlas")).unwrap();
        fs::write(
            outside.path().join("tasks.json"),
            r#"{"private":{"prompt":"outside"}}"#,
        )
        .unwrap();
        symlink(outside.path(), store_dir(root.path())).unwrap();
        assert!(list(root.path()).is_err());
        assert!(get(root.path(), "private").is_err());
        assert!(set(root.path(), "new", "safe").is_err());
        assert!(!outside.path().join("lock").exists());
    }
    #[cfg(unix)]
    #[test]
    fn legacy_file_symlink_is_rejected_before_read_or_migration() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join(".atlas")).unwrap();
        let private = outside.path().join("private.json");
        fs::write(&private, r#"{"hidden":{"prompt":"secret"}}"#).unwrap();
        symlink(&private, legacy_path(root.path())).unwrap();
        assert!(list(root.path()).is_err());
        assert!(change(root.path(), "hidden", "wrong").is_err());
        assert!(private.exists());
    }
    #[test]
    fn invalid_store_data_is_an_error_not_an_empty_success() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(store_dir(root.path())).unwrap();
        fs::write(store_path(root.path()), b"invalid json").unwrap();
        assert!(list(root.path()).is_err());
        assert!(set(root.path(), "name", "prompt").is_err());
    }
}
