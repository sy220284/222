//! Local configuration, provider-agnostic registry and setup utilities.
#![allow(dead_code)]
use anyhow::{Context, Result};
use chrono::{Datelike, Local, NaiveDate};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    future::Future,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

// src/constants/common.ts
static SESSION_DATE: OnceLock<String> = OnceLock::new();
pub fn local_iso_date(override_date: Option<&str>) -> String {
    if let Some(raw) = override_date.filter(|s| !s.is_empty()) {
        return raw.into();
    }
    Local::now().format("%Y-%m-%d").to_string()
}
pub fn session_start_date(override_date: Option<&str>) -> &'static str {
    SESSION_DATE.get_or_init(|| local_iso_date(override_date))
}
pub fn local_month_year(override_date: Option<&str>) -> String {
    let date = override_date
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or_else(|| Local::now().date_naive());
    date.format("%B %Y").to_string()
}
// src/constants/toolLimits.ts
pub const DEFAULT_MAX_RESULT_SIZE_CHARS: usize = 50_000;
pub const MAX_TOOL_RESULT_TOKENS: usize = 100_000;
pub const BYTES_PER_TOKEN: usize = 4;
pub const MAX_TOOL_RESULT_BYTES: usize = MAX_TOOL_RESULT_TOKENS * BYTES_PER_TOKEN;
pub const MAX_TOOL_RESULTS_PER_MESSAGE_CHARS: usize = 200_000;
pub const TOOL_SUMMARY_MAX_LENGTH: usize = 50;
// src/constants/systemPromptSections.ts
#[derive(Default)]
pub struct SystemPromptSections {
    cache: Mutex<HashMap<String, Option<String>>>,
}
impl SystemPromptSections {
    pub async fn resolve<F, Fut>(&self, name: &str, uncached: bool, compute: F) -> Option<String>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Option<String>>,
    {
        if !uncached {
            if let Some(existing) = self.cache.lock().unwrap().get(name).cloned() {
                return existing;
            }
        }
        let value = compute().await;
        self.cache
            .lock()
            .unwrap()
            .insert(name.into(), value.clone());
        value
    }
    pub fn clear(&self) {
        self.cache.lock().unwrap().clear()
    }
    pub fn entry_count(&self) -> usize {
        self.cache.lock().unwrap().len()
    }
}
// src/constants/xml.ts
pub const COMMAND_NAME_TAG: &str = "command-name";
pub const COMMAND_MESSAGE_TAG: &str = "command-message";
pub const COMMAND_ARGS_TAG: &str = "command-args";
pub const BASH_INPUT_TAG: &str = "bash-input";
pub const BASH_STDOUT_TAG: &str = "bash-stdout";
pub const BASH_STDERR_TAG: &str = "bash-stderr";
pub const LOCAL_COMMAND_STDOUT_TAG: &str = "local-command-stdout";
pub const LOCAL_COMMAND_STDERR_TAG: &str = "local-command-stderr";
pub const TASK_NOTIFICATION_TAG: &str = "task-notification";
pub const TASK_ID_TAG: &str = "task-id";
pub const TOOL_USE_ID_TAG: &str = "tool-use-id";
pub const TEAMMATE_MESSAGE_TAG: &str = "teammate-message";
pub const CHANNEL_MESSAGE_TAG: &str = "channel-message";
pub const COMMON_HELP_ARGS: &[&str] = &["help", "-h", "--help"];
pub fn xml_tag(name: &str, text: &str) -> String {
    format!("<{name}>{}</{name}>", crate::xml_escape::escape_xml(text))
}
// src/constants/apiLimits.ts
pub const API_IMAGE_MAX_BASE64_SIZE: usize = 5 * 1024 * 1024;
pub const IMAGE_TARGET_RAW_SIZE: usize = API_IMAGE_MAX_BASE64_SIZE * 3 / 4;
pub const IMAGE_MAX_WIDTH: u32 = 2000;
pub const IMAGE_MAX_HEIGHT: u32 = 2000;
pub const PDF_TARGET_RAW_SIZE: usize = 20 * 1024 * 1024;
pub const API_PDF_MAX_PAGES: usize = 100;
pub const PDF_EXTRACT_SIZE_THRESHOLD: usize = 3 * 1024 * 1024;
pub const PDF_MAX_EXTRACT_SIZE: usize = 100 * 1024 * 1024;
pub const PDF_MAX_PAGES_PER_READ: usize = 20;
pub const PDF_AT_MENTION_INLINE_THRESHOLD: usize = 10;
pub const API_MAX_MEDIA_PER_REQUEST: usize = 100;
// src/utils/authPortable.ts (keychain vendor account cleanup intentionally excluded)
pub fn normalize_api_key_for_config(api_key: &str) -> String {
    let units = api_key.encode_utf16().collect::<Vec<_>>();
    let suffix = &units[units.len().saturating_sub(20)..];
    String::from_utf16_lossy(suffix)
}
// src/utils/bundledMode.ts (Rust native compiled mode, not Bun runtime)
pub fn is_native_binary() -> bool {
    true
}
pub fn is_bundled_runtime(asset_count: usize) -> bool {
    asset_count > 0
}
// src/utils/cwd.ts
tokio::task_local! {static CWD_OVERRIDE:PathBuf;}
pub async fn run_with_cwd_override<T, F: Future<Output = T>>(cwd: PathBuf, future: F) -> T {
    CWD_OVERRIDE.scope(cwd, future).await
}
pub fn current_workspace_cwd() -> Result<PathBuf> {
    CWD_OVERRIDE
        .try_with(Clone::clone)
        .ok()
        .map(Ok)
        .unwrap_or_else(|| std::env::current_dir().map_err(Into::into))
}
// src/utils/execSyncWrapper.ts (unrestricted shell calls must pass authorization gate)
pub fn exec_sync(command: &str, cwd: &Path) -> Result<(i32, String, String)> {
    let output = crate::exec_sync_compat::run_shell(
        command,
        crate::exec_sync_compat::SyncOptions {
            cwd: Some(cwd.to_path_buf()),
            ..Default::default()
        },
    )?;
    Ok((
        output.code,
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    ))
}
// src/utils/github/ghAuthStatus.ts
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GitHubAuthStatus {
    NotInstalled,
    NotAuthenticated,
    Authenticated,
}
pub async fn gh_auth_status() -> GitHubAuthStatus {
    let Some(exe) = crate::shell_runtime::which("gh") else {
        return GitHubAuthStatus::NotInstalled;
    };
    let mut cmd = tokio::process::Command::new(exe);
    cmd.args(["auth", "token"]);
    let output = crate::exec_file::execute(
        cmd,
        crate::exec_file::Options {
            timeout: std::time::Duration::from_secs(6),
            preserve_output_on_error: false,
            ..Default::default()
        },
    )
    .await;
    if output.success() {
        GitHubAuthStatus::Authenticated
    } else {
        GitHubAuthStatus::NotAuthenticated
    }
}
// src/utils/immediateCommand.ts
pub fn inference_command_immediate(policy_gate: bool, explicit_setting: Option<bool>) -> bool {
    explicit_setting.unwrap_or(policy_gate)
}
// src/utils/ink.ts
pub fn ink_color(color: Option<&str>, theme_mapping: &HashMap<String, String>) -> String {
    let Some(color) = color.filter(|v| !v.is_empty()) else {
        return "cyan_FOR_SUBAGENTS_ONLY".into();
    };
    if let Some(theme) = theme_mapping.get(color) {
        return theme.clone();
    }
    format!("ansi:{color}")
}
// src/utils/lazySchema.ts
pub struct LazySchema<T> {
    value: OnceLock<T>,
}
impl<T> Default for LazySchema<T> {
    fn default() -> Self {
        Self {
            value: OnceLock::new(),
        }
    }
}
impl<T> LazySchema<T> {
    pub fn get_or_init(&self, factory: impl FnOnce() -> T) -> &T {
        self.value.get_or_init(factory)
    }
}
// src/utils/model/aliases.ts — aliases come from user-owned provider profiles.
pub fn is_model_alias(input: &str, configured: &BTreeMap<String, String>) -> bool {
    configured.contains_key(input)
}
pub fn resolve_model_alias<'a>(
    input: &'a str,
    configured: &'a BTreeMap<String, String>,
) -> &'a str {
    configured.get(input).map(String::as_str).unwrap_or(input)
}
pub fn is_model_family_alias(input: &str, configured_families: &HashSet<String>) -> bool {
    configured_families.contains(input)
}
// src/utils/modifiers.ts
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ModifierKey {
    Shift,
    Command,
    Control,
    Option,
}
#[derive(Debug, Default, Clone)]
pub struct ModifierState {
    pressed: HashSet<ModifierKey>,
}
impl ModifierState {
    pub fn set(&mut self, key: ModifierKey, down: bool) {
        if down {
            self.pressed.insert(key);
        } else {
            self.pressed.remove(&key);
        }
    }
    pub fn pressed(&self, key: ModifierKey) -> bool {
        self.pressed.contains(&key)
    }
    pub fn clear(&mut self) {
        self.pressed.clear()
    }
}
// src/utils/nativeInstaller/index.ts
pub fn native_installer_path(bin_dir: &Path, program: &str) -> PathBuf {
    #[cfg(windows)]
    {
        bin_dir.join(format!("{program}.exe"))
    }
    #[cfg(not(windows))]
    {
        bin_dir.join(program)
    }
}
pub fn native_installer_present(bin_dir: &Path, program: &str) -> bool {
    native_installer_path(bin_dir, program).is_file()
}
// src/utils/plugins/managedPlugins.ts
pub fn managed_plugin_names(enabled: &HashMap<String, Value>) -> Option<HashSet<String>> {
    let names = enabled
        .iter()
        .filter_map(|(id, value)| {
            if !value.is_boolean() {
                return None;
            }
            let (name, marketplace) = id.split_once('@')?;
            if name.is_empty() || marketplace.is_empty() {
                None
            } else {
                Some(name.to_owned())
            }
        })
        .collect::<HashSet<_>>();
    (!names.is_empty()).then_some(names)
}
// src/utils/plugins/officialMarketplace.ts — neutral configurable marketplace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultMarketplace {
    pub id: String,
    pub repository: String,
}
pub fn default_marketplace(id: &str, repo: &str) -> Result<DefaultMarketplace> {
    if id.trim().is_empty() || id.contains(['/', '\\']) {
        anyhow::bail!("Invalid marketplace name")
    }
    if !(repo.starts_with("https://") || repo.starts_with("git@")) {
        anyhow::bail!("Marketplace URL must be HTTPS or SSH")
    }
    Ok(DefaultMarketplace {
        id: id.into(),
        repository: repo.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn session_date_limits_and_marketplaces() {
        assert_eq!(local_iso_date(Some("2026-10-29")), "2026-10-29");
        assert_eq!(local_month_year(Some("2026-10-29")), "October 2026");
        assert_eq!(MAX_TOOL_RESULT_BYTES, 400_000);
        assert_eq!(normalize_api_key_for_config("short-key"), "short-key");
        assert!(default_marketplace("self-hosted", "https://example.org/repo").is_ok());
        assert!(default_marketplace("invalid/path", "https://example.org").is_err());
        let enabled = HashMap::from([
            ("alpha@market".into(), json!(true)),
            ("legacy".into(), json!(["enabled"])),
        ]);
        assert_eq!(
            managed_plugin_names(&enabled).unwrap(),
            HashSet::from(["alpha".into()])
        );
    }
    #[tokio::test]
    async fn system_prompt_section_caching_and_context() {
        let cache = SystemPromptSections::default();
        let count = std::sync::atomic::AtomicUsize::new(0);
        let a = cache
            .resolve("tools", false, || async { Some("initial".into()) })
            .await;
        let b = cache
            .resolve("tools", false, || {
                count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                async { Some("changed".into()) }
            })
            .await;
        assert_eq!(a, b);
        assert_eq!(count.load(std::sync::atomic::Ordering::Relaxed), 0);
        let c = cache
            .resolve("tools", true, || async { Some("changed".into()) })
            .await;
        assert_eq!(c, Some("changed".into()));
        assert_eq!(
            run_with_cwd_override("/tmp".into(), async { current_workspace_cwd().unwrap() }).await,
            PathBuf::from("/tmp")
        );
    }
}
