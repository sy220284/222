//! Source parity: permissionsLoader.ts, settings/constants.ts and settings/settings.ts.
//! Paths and policy injection are vendor-neutral; incomplete UI/MDM behavior is audited.
use crate::permission_rules::{
    format_rule, parse_rule, valid_stored_rule, Behavior, RuleSetting, RuleValue,
};
use anyhow::{bail, Context, Result};
use fs2::FileExt;
use serde_json::{Map, Value};
use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug)]
pub enum EditableSource {
    User,
    Project,
    Local,
}

#[derive(Clone, Debug)]
pub struct SettingsPaths {
    workspace: PathBuf,
    user: PathBuf,
    project: PathBuf,
    local: PathBuf,
    policy: Option<PathBuf>,
}

impl SettingsPaths {
    pub fn for_workspace(workspace: &Path) -> Result<Self> {
        let workspace = workspace
            .canonicalize()
            .context("Resolve permission workspace")?;
        let user_root = dirs::config_dir()
            .context("Cannot locate user settings directory")?
            .join("atlas-agent");
        Ok(Self {
            project: workspace.join(".atlas/settings.json"),
            local: workspace.join(".atlas/settings.local.json"),
            workspace,
            user: std::env::var_os("ATLAS_USER_SETTINGS")
                .map(PathBuf::from)
                .unwrap_or_else(|| user_root.join("settings.json")),
            policy: std::env::var_os("ATLAS_POLICY_SETTINGS").map(PathBuf::from),
        })
    }

    fn path(&self, source: EditableSource) -> &Path {
        match source {
            EditableSource::User => &self.user,
            EditableSource::Project => &self.project,
            EditableSource::Local => &self.local,
        }
    }

    fn read_json_raw(path: &Path) -> Result<Option<Value>> {
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error).with_context(|| format!("Inspect {}", path.display())),
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            bail!(
                "Permission settings path must be a regular file: {}",
                path.display()
            );
        }
        let content = fs::read_to_string(path)
            .with_context(|| format!("Read permission settings {}", path.display()))?;
        if content.trim().is_empty() {
            return Ok(Some(Value::Object(Map::new())));
        }
        let value: Value = serde_json::from_str(crate::json_read::strip_bom(&content))
            .with_context(|| format!("Parse permission settings {}", path.display()))?;
        Ok(Some(value))
    }

    /// Editing requires a valid object and must never replace a malformed
    /// settings root. Execution may skip invalid sources separately.
    fn read_json(path: &Path) -> Result<Option<Value>> {
        let value = Self::read_json_raw(path)?;
        if value.as_ref().is_some_and(|value| !value.is_object()) {
            bail!(
                "Permission settings must be a JSON object: {}",
                path.display()
            );
        }
        Ok(value)
    }

    /// The source settings loader reports malformed JSON and skips that
    /// source during normal execution. Editing uses strict read_json so a
    /// broken file can never be silently replaced by a permission update.
    fn read_json_for_execution(path: &Path) -> Result<Option<Value>> {
        match Self::read_json_raw(path) {
            Ok(Some(value)) if !value.is_object() => {
                eprintln!(
                    "Ignoring invalid settings root in permission source {}",
                    path.display()
                );
                Ok(None)
            }
            Ok(settings) => Ok(settings),
            Err(error) if error.downcast_ref::<serde_json::Error>().is_some() => {
                eprintln!(
                    "Ignoring invalid JSON in permission settings {}: {error:#}",
                    path.display()
                );
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    fn dropin_directory(base: &Path) -> PathBuf {
        std::env::var_os("ATLAS_POLICY_DROPIN_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                base.parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join("managed-settings.d")
            })
    }

    /// Source settings.ts::getManagedFileSettingsPresence: report separately
    /// whether the managed base has valid content and whether any eligible
    /// drop-in files exist. The status view must not disclose policy contents.
    pub fn managed_file_presence(&self) -> (bool, bool) {
        let Some(base) = self.policy.as_ref() else {
            return (false, false);
        };
        let has_base = Self::read_json_for_execution(base)
            .ok()
            .flatten()
            .and_then(|value| value.as_object().map(|object| !object.is_empty()))
            .unwrap_or(false);
        let has_dropins = fs::read_dir(Self::dropin_directory(base))
            .ok()
            .is_some_and(|entries| {
                entries.flatten().any(|entry| {
                    let name = entry.file_name();
                    let Some(name) = name.to_str() else { return false };
                    !name.starts_with('.')
                        && name.ends_with(".json")
                        && entry
                            .file_type()
                            .is_ok_and(|kind| kind.is_file() || kind.is_symlink())
                })
            });
        (has_base, has_dropins)
    }

    /// Original settings.ts::loadManagedFileSettings: load the managed base
    /// first, then alphabetically ordered managed-settings.d/*.json fragments.
    /// Permission arrays concatenate without duplicates; later scalar values
    /// override earlier ones. An invalid fragment cannot discard valid policy.
    fn load_policy(&self) -> Result<Option<Value>> {
        let Some(base) = self.policy.as_ref() else {
            return Ok(None);
        };
        let mut merged = Value::Object(Map::new());
        if let Some(settings) = Self::read_json_for_execution(base)? {
            merge_policy_values(&mut merged, settings);
        }
        let dropin_dir = Self::dropin_directory(base);
        let entries = match fs::read_dir(&dropin_dir) {
            Ok(entries) => entries,
            Err(error) if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
            ) => {
                return Ok((merged != Value::Object(Map::new())).then_some(merged));
            }
            Err(error) => {
                eprintln!(
                    "Unable to read managed permission drop-ins {}: {error}",
                    dropin_dir.display()
                );
                return Ok((merged != Value::Object(Map::new())).then_some(merged));
            }
        };
        let mut paths = Vec::new();
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    eprintln!("Unable to inspect managed permission drop-in: {error}");
                    continue;
                }
            };
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            if name.starts_with('.') || !name.ends_with(".json") {
                continue;
            }
            match entry.file_type() {
                Ok(kind) if kind.is_file() || kind.is_symlink() => paths.push(entry.path()),
                Ok(_) => {}
                Err(error) => eprintln!(
                    "Unable to inspect managed permission drop-in {}: {error}",
                    entry.path().display()
                ),
            }
        }
        paths.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
        for path in paths {
            match Self::read_json_for_execution(&path) {
                Ok(Some(settings)) => merge_policy_values(&mut merged, settings),
                Ok(None) => {}
                Err(error) => eprintln!(
                    "Ignoring unreadable managed permission drop-in {}: {error:#}",
                    path.display()
                ),
            }
        }
        Ok((merged != Value::Object(Map::new())).then_some(merged))
    }

    fn managed_only(policy: Option<&Value>) -> bool {
        policy
            .and_then(|value| value.get("allowManagedPermissionRulesOnly"))
            .and_then(Value::as_bool)
            == Some(true)
    }

    fn append_rules(settings: Option<&Value>, result: &mut Vec<RuleSetting>) -> Result<()> {
        let Some(permissions) = settings.and_then(|value| value.get("permissions")) else {
            return Ok(());
        };
        // settings.ts::parseSettingsFile returns no settings for a source
        // whose permissions shape fails schema validation. Do not abort
        // startup or discard valid rules from other settings sources.
        let Some(permissions) = permissions.as_object() else {
            eprintln!("Ignoring invalid permissions object in settings source");
            return Ok(());
        };
        for behavior in ["allow", "deny", "ask"] {
            if permissions
                .get(behavior)
                .is_some_and(|values| !values.is_array())
            {
                eprintln!("Ignoring settings source with non-array permission {behavior}");
                return Ok(());
            }
        }
        for behavior in ["allow", "deny", "ask"] {
            if let Some(values) = permissions.get(behavior) {
                let values = values.as_array().expect("permission array validated");
                for value in values {
                    let Some(raw) = value.as_str() else {
                        eprintln!("Skipping non-string permission {behavior} entry");
                        continue;
                    };
                    if !valid_stored_rule(raw) {
                        eprintln!("Skipping invalid permission {behavior} rule: {raw}");
                        continue;
                    }
                    result.push(RuleSetting {
                        behavior: behavior.to_owned(),
                        rule: raw.to_owned(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Load user, project, local, config/flag and managed rules. Managed-only
    /// excludes every non-policy source, including existing config rules.
    pub fn load(&self, configured: &[RuleSetting]) -> Result<Vec<RuleSetting>> {
        let policy = self.load_policy()?;
        let mut result = Vec::new();
        if Self::managed_only(policy.as_ref()) {
            Self::append_rules(policy.as_ref(), &mut result)?;
            return Ok(result);
        }
        for path in [&self.user, &self.project, &self.local] {
            if path == &self.project || path == &self.local {
                if let Some(parent) = path.parent() {
                    if parent.exists() && !parent.canonicalize()?.starts_with(&self.workspace) {
                        bail!("Project permission settings escape the workspace");
                    }
                }
            }
            let settings = Self::read_json_for_execution(path)?;
            Self::append_rules(settings.as_ref(), &mut result)?;
        }
        result.extend_from_slice(configured);
        Self::append_rules(policy.as_ref(), &mut result)?;
        Ok(result)
    }

    fn validate_edit_path(&self, path: &Path) -> Result<()> {
        let parent = path
            .parent()
            .context("Permission settings have no parent")?;
        fs::create_dir_all(parent)?;
        if path == self.project || path == self.local {
            let canonical = parent.canonicalize()?;
            if !canonical.starts_with(&self.workspace) {
                bail!("Project permission settings escape the workspace");
            }
        }
        if let Ok(metadata) = fs::symlink_metadata(path) {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                bail!("Refusing to replace non-regular permission settings");
            }
        }
        Ok(())
    }

    fn write_atomic(path: &Path, data: &Value) -> Result<()> {
        let parent = path
            .parent()
            .context("Permission settings have no parent")?;
        let temporary = parent.join(format!(".permissions-{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| -> Result<()> {
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temporary)?;
            serde_json::to_writer_pretty(&mut file, data)?;
            file.write_all(b"\n")?;
            file.sync_all()?;
            fs::rename(&temporary, path)?;
            if let Ok(directory) = File::open(parent) {
                directory.sync_all()?;
            }
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }

    /// Returns None when a managed policy forbids edits, Some(changed) otherwise.
    fn edit<F>(&self, source: EditableSource, change: F) -> Result<Option<bool>>
    where
        F: FnOnce(&mut Map<String, Value>) -> Result<bool>,
    {
        let policy = self.load_policy()?;
        if Self::managed_only(policy.as_ref()) {
            return Ok(None);
        }
        let path = self.path(source);
        self.validate_edit_path(path)?;
        let lock_path = path.with_extension(format!(
            "{}.lock",
            path.extension().and_then(|s| s.to_str()).unwrap_or("json")
        ));
        if fs::symlink_metadata(&lock_path).is_ok_and(|metadata| metadata.file_type().is_symlink())
        {
            bail!("Refusing symlinked permission settings lock");
        }
        let lock = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&lock_path)?;
        lock.lock_exclusive()?;
        let mut settings = Self::read_json(path)?.unwrap_or_else(|| Value::Object(Map::new()));
        let object = settings
            .as_object_mut()
            .context("Permission settings must be an object")?;
        let changed = change(object)?;
        if changed {
            Self::write_atomic(path, &settings)?;
        }
        Ok(Some(changed))
    }

    pub fn add(
        &self,
        source: EditableSource,
        behavior: Behavior,
        rules: &[RuleValue],
    ) -> Result<bool> {
        self.edit(source, |settings| {
            if rules.is_empty() {
                return Ok(false);
            }
            let permissions = settings
                .entry("permissions")
                .or_insert_with(|| Value::Object(Map::new()))
                .as_object_mut()
                .context("permissions must be an object")?;
            let values = permissions
                .entry(behavior_name(behavior))
                .or_insert_with(|| Value::Array(Vec::new()))
                .as_array_mut()
                .context("Permission rule list must be an array")?;
            let existing: HashSet<String> = values
                .iter()
                .map(|value| {
                    let raw = value.as_str().context("Permission rule must be a string")?;
                    Ok(format_rule(&parse_rule(raw)))
                })
                .collect::<Result<_>>()?;
            let mut known = existing;
            let mut changed = false;
            for rule in rules {
                let formatted = format_rule(rule);
                if known.insert(formatted.clone()) {
                    values.push(Value::String(formatted));
                    changed = true;
                }
            }
            Ok(changed)
        })
        .map(|allowed| allowed.is_some())
    }

    pub fn delete(
        &self,
        source: EditableSource,
        behavior: Behavior,
        rule: &RuleValue,
    ) -> Result<bool> {
        let target = format_rule(rule);
        self.edit(source, |settings| {
            let Some(values) = settings
                .get_mut("permissions")
                .and_then(Value::as_object_mut)
                .and_then(|permissions| permissions.get_mut(behavior_name(behavior)))
                .and_then(Value::as_array_mut)
            else {
                return Ok(false);
            };
            let old_len = values.len();
            values.retain(|value| {
                value
                    .as_str()
                    .is_none_or(|raw| format_rule(&parse_rule(raw)) != target)
            });
            Ok(values.len() != old_len)
        })
        .map(|changed| changed.unwrap_or(false))
    }
}

/// Lodash mergeWith + settingsMergeCustomizer equivalent for managed policy:
/// merge nested objects and concatenate/deduplicate arrays in source order.
fn merge_policy_values(target: &mut Value, source: Value) {
    match (target, source) {
        (Value::Object(existing), Value::Object(incoming)) => {
            for (key, value) in incoming {
                merge_policy_values(existing.entry(key).or_insert(Value::Null), value);
            }
        }
        (Value::Array(existing), Value::Array(incoming)) => {
            for value in incoming {
                if !existing.contains(&value) {
                    existing.push(value);
                }
            }
        }
        (target, incoming) => *target = incoming,
    }
}

fn behavior_name(behavior: Behavior) -> &'static str {
    match behavior {
        Behavior::Allow => "allow",
        Behavior::Deny => "deny",
        Behavior::Ask => "ask",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permission_rules::{compile_rules, policy_for};
    use serde_json::json;
    use std::sync::Arc;

    fn paths(root: &Path) -> SettingsPaths {
        SettingsPaths {
            workspace: root.canonicalize().unwrap(),
            user: root.join("user/settings.json"),
            project: root.join(".atlas/settings.json"),
            local: root.join(".atlas/settings.local.json"),
            policy: Some(root.join("policy.json")),
        }
    }

    fn write(path: &Path, value: &Value) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
    }

    #[test]
    fn policy_only_disables_all_nonmanaged_rules_and_edits() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        write(
            &paths.user,
            &json!({"permissions":{"allow":["Bash(touch unsafe)"]}}),
        );
        write(
            &paths.project,
            &json!({"permissions":{"allow":["Bash(touch unsafe)"]}}),
        );
        write(
            paths.policy.as_ref().unwrap(),
            &json!({
                "allowManagedPermissionRulesOnly":true,
                "permissions":{"deny":["Bash(touch unsafe)"]}
            }),
        );
        let loaded = paths
            .load(&[RuleSetting {
                behavior: "allow".into(),
                rule: "Bash(touch unsafe)".into(),
            }])
            .unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].behavior, "deny");
        assert!(!paths
            .add(
                EditableSource::Local,
                Behavior::Allow,
                &[parse_rule("Bash(touch unsafe)")]
            )
            .unwrap());
        assert!(!paths.local.exists());
    }

    #[test]
    fn source_rules_reach_real_shell_permission_gate() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        write(
            &paths.project,
            &json!({"permissions":{"allow":["Bash(touch allowed)"]}}),
        );
        write(
            paths.policy.as_ref().unwrap(),
            &json!({"permissions":{"deny":["Bash(touch blocked)"]}}),
        );
        let rules = compile_rules(&paths.load(&[]).unwrap()).unwrap();
        assert_eq!(
            policy_for(&rules, "run_command", &json!({"command":"touch allowed"})).unwrap(),
            Some(Behavior::Allow)
        );
        assert_eq!(
            policy_for(&rules, "run_command", &json!({"command":"touch blocked"})).unwrap(),
            Some(Behavior::Deny)
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn malformed_source_json_does_not_abort_other_valid_permission_sources() {
        use crate::{
            protocol::{FunctionCall, ToolCall},
            tools::{Approval, ToolExecutor},
        };
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        write(
            &paths.user,
            &json!({"permissions":{"allow":["Bash(touch allowed)"]}}),
        );
        fs::create_dir_all(paths.project.parent().unwrap()).unwrap();
        fs::write(&paths.project, "{invalid json").unwrap();
        write(
            &paths.local,
            &json!({"permissions":{"deny":["Bash(touch forbidden)"]}}),
        );
        let rules = compile_rules(&paths.load(&[]).unwrap()).unwrap();
        let executor = ToolExecutor::new(root.path().canonicalize().unwrap(), Approval::Ask, false)
            .with_permission_rules(rules);
        let call = |command: &str| ToolCall {
            id: "source-recovery-test".into(),
            kind: "function".into(),
            function: FunctionCall {
                name: "run_command".into(),
                arguments: json!({"command":command}).to_string(),
            },
        };
        let allowed = executor.execute(&call("touch allowed")).await;
        assert!(allowed.contains("Exit: exit status: 0"), "{allowed}");
        assert!(root.path().join("allowed").exists());
        let denied = executor.execute(&call("touch forbidden")).await;
        assert!(denied.starts_with("ERROR:"), "{denied}");
        assert!(!root.path().join("forbidden").exists());
        assert!(paths
            .add(
                EditableSource::Project,
                Behavior::Allow,
                &[parse_rule("Bash(touch unexpected)")]
            )
            .is_err());
        assert_eq!(fs::read_to_string(&paths.project).unwrap(), "{invalid json");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn invalid_permission_schema_skips_only_that_source_and_keeps_shell_working() {
        // settings.ts::parseSettingsFile rejects an invalid settings schema
        // without terminating other sources' permission loading.
        use crate::{
            protocol::{FunctionCall, ToolCall},
            tools::{Approval, ToolExecutor},
        };
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        write(
            &paths.user,
            &json!({"permissions":{"allow":"Bash(touch should_not_run)",
                "deny":["Bash(touch ignored_user_deny)"]}}),
        );
        write(
            &paths.project,
            &json!({"permissions":{"allow":["Bash(touch permitted)"]}}),
        );
        write(
            &paths.local,
            &json!({"permissions":{"deny":["Bash(touch forbidden)"]}}),
        );
        let loaded = paths.load(&[]).unwrap();
        assert_eq!(
            loaded.len(),
            2,
            "invalid source must not leak partial rules"
        );
        let executor = ToolExecutor::new(root.path().canonicalize().unwrap(), Approval::Ask, false)
            .with_permission_rules(compile_rules(&loaded).unwrap());
        let call = |command: &str| ToolCall {
            id: "invalid-permissions-schema".into(),
            kind: "function".into(),
            function: FunctionCall {
                name: "run_command".into(),
                arguments: json!({"command":command}).to_string(),
            },
        };
        let allowed = executor.execute(&call("touch permitted")).await;
        assert!(allowed.contains("Exit: exit status: 0"), "{allowed}");
        assert!(root.path().join("permitted").exists());
        let denied = executor.execute(&call("touch forbidden")).await;
        assert!(denied.starts_with("ERROR:"), "{denied}");
        assert!(!root.path().join("forbidden").exists());
        let invalid = executor.execute(&call("touch should_not_run")).await;
        assert!(invalid.starts_with("ERROR:"), "{invalid}");
        assert!(!root.path().join("should_not_run").exists());

        // Valid JSON with an invalid root type must be skipped during
        // execution, but never silently replaced by a permission edit.
        fs::write(&paths.user, "[]").unwrap();
        assert_eq!(paths.load(&[]).unwrap().len(), 2);
        assert!(paths
            .add(
                EditableSource::User,
                Behavior::Allow,
                &[parse_rule("Bash(touch should_not_run)")]
            )
            .is_err());
        assert_eq!(fs::read_to_string(&paths.user).unwrap(), "[]");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn invalid_rules_are_filtered_without_discarding_valid_siblings() {
        use crate::{
            protocol::{FunctionCall, ToolCall},
            tools::{Approval, ToolExecutor},
        };
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        write(
            &paths.project,
            &json!({"permissions":{"allow":[
                17,
                "Bash()",
                "Bash(touch allowed)",
                "Bash(npm:* run)",
                "WebFetch(https://example.com)"
            ],"deny":["Bash(touch forbidden)"]}}),
        );
        let loaded = paths.load(&[]).unwrap();
        assert_eq!(loaded.len(), 2);
        let executor = ToolExecutor::new(root.path().canonicalize().unwrap(), Approval::Ask, false)
            .with_permission_rules(compile_rules(&loaded).unwrap());
        let call = |command: &str| ToolCall {
            id: "invalid-rule-filter".into(),
            kind: "function".into(),
            function: FunctionCall {
                name: "run_command".into(),
                arguments: json!({"command":command}).to_string(),
            },
        };
        let allowed = executor.execute(&call("touch allowed")).await;
        assert!(allowed.contains("Exit: exit status: 0"), "{allowed}");
        assert!(root.path().join("allowed").exists());
        let denied = executor.execute(&call("touch forbidden")).await;
        assert!(denied.starts_with("ERROR:"), "{denied}");
        assert!(!root.path().join("forbidden").exists());
    }

    #[test]
    fn append_delete_preserves_unknown_fields_and_normalizes_legacy_names() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        write(
            &paths.local,
            &json!({
                "hooks":{"invalid-but-preserved":true},
                "permissions":{"allow":["KillShell(123)"],"custom":["keep"]}
            }),
        );
        let rule = parse_rule("TaskStop(123)");
        assert!(paths
            .add(EditableSource::Local, Behavior::Allow, &[rule.clone()])
            .unwrap());
        let before = SettingsPaths::read_json(&paths.local).unwrap().unwrap();
        assert_eq!(before["permissions"]["allow"], json!(["KillShell(123)"]));
        assert_eq!(before["hooks"]["invalid-but-preserved"], true);
        assert!(paths
            .delete(EditableSource::Local, Behavior::Allow, &rule)
            .unwrap());
        let after = SettingsPaths::read_json(&paths.local).unwrap().unwrap();
        assert_eq!(after["permissions"]["allow"], json!([]));
        assert_eq!(after["permissions"]["custom"], json!(["keep"]));
    }

    #[test]
    fn corrupt_settings_cannot_be_overwritten_by_add() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        fs::create_dir_all(paths.local.parent().unwrap()).unwrap();
        fs::write(&paths.local, "{bad json").unwrap();
        assert!(paths
            .add(
                EditableSource::Local,
                Behavior::Allow,
                &[parse_rule("Bash(ls)")]
            )
            .is_err());
        assert_eq!(fs::read_to_string(&paths.local).unwrap(), "{bad json");
    }

    #[test]
    fn concurrent_writers_preserve_every_distinct_rule() {
        let root = tempfile::tempdir().unwrap();
        let paths = Arc::new(paths(root.path()));
        let handles: Vec<_> = (0..16)
            .map(|id| {
                let paths = Arc::clone(&paths);
                std::thread::spawn(move || {
                    let rule = parse_rule(&format!("Bash(touch file{id})"));
                    paths
                        .add(EditableSource::Local, Behavior::Allow, &[rule])
                        .unwrap();
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }
        assert_eq!(paths.load(&[]).unwrap().len(), 16);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn persisted_policy_blocks_actual_shell_and_filesystem_side_effect() {
        use crate::{
            protocol::{FunctionCall, ToolCall},
            tools::{Approval, ToolExecutor},
        };
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        write(
            &paths.project,
            &json!({"permissions":{"allow":["Bash(touch allowed)"]}}),
        );
        write(
            paths.policy.as_ref().unwrap(),
            &json!({"permissions":{"deny":["Bash(touch blocked)"]}}),
        );
        let rules = compile_rules(&paths.load(&[]).unwrap()).unwrap();
        let executor = ToolExecutor::new(root.path().canonicalize().unwrap(), Approval::Ask, false)
            .with_permission_rules(rules);
        let make_call = |command: &str| ToolCall {
            id: "permission-source-test".into(),
            kind: "function".into(),
            function: FunctionCall {
                name: "run_command".into(),
                arguments: json!({"command":command}).to_string(),
            },
        };
        let denied = executor.execute(&make_call("touch blocked")).await;
        assert!(denied.starts_with("ERROR:"), "{denied}");
        assert!(!root.path().join("blocked").exists());
        let allowed = executor.execute(&make_call("touch allowed")).await;
        assert!(allowed.contains("Exit: exit status: 0"), "{allowed}");
        assert!(root.path().join("allowed").exists());
    }

    #[cfg(unix)]
    #[test]
    fn project_settings_symlink_cannot_escape_workspace() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        symlink(outside.path(), root.path().join(".atlas")).unwrap();
        assert!(paths
            .add(
                EditableSource::Project,
                Behavior::Allow,
                &[parse_rule("Bash(touch escaped)")]
            )
            .is_err());
        assert!(!outside.path().join("settings.json").exists());
    }
}
