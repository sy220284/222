//! Portable settings IO policy, tool schema cache and command specs.
#![allow(dead_code)]
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

// src/utils/settings/allErrors.ts
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingsWithErrors {
    pub settings: Value,
    pub errors: Vec<String>,
}
pub fn merge_settings_and_mcp_errors(
    settings: Value,
    mut validation_errors: Vec<String>,
    scope_errors: &HashMap<String, Vec<String>>,
) -> SettingsWithErrors {
    for scope in ["user", "project", "local"] {
        if let Some(errors) = scope_errors.get(scope) {
            validation_errors.extend(errors.iter().cloned());
        }
    }
    SettingsWithErrors {
        settings,
        errors: validation_errors,
    }
}
// src/utils/settings/internalWrites.ts
#[derive(Default)]
pub struct InternalWriteMarks {
    entries: Mutex<HashMap<PathBuf, Instant>>,
}
impl InternalWriteMarks {
    pub fn mark(&self, path: &Path) {
        self.entries
            .lock()
            .unwrap()
            .insert(path.into(), Instant::now());
    }
    pub fn consume(&self, path: &Path, window: Duration) -> bool {
        let mut entries = self.entries.lock().unwrap();
        if let Some(mark) = entries.get(path).copied() {
            if mark.elapsed() < window {
                entries.remove(path);
                return true;
            }
        }
        false
    }
    pub fn clear(&self) {
        self.entries.lock().unwrap().clear()
    }
}
// src/utils/settings/managedPath.ts — organization policy, vendor-neutral.
pub fn managed_file_path(configured: Option<&Path>) -> PathBuf {
    if let Some(path) = configured {
        return path.into();
    }
    if cfg!(target_os = "windows") {
        PathBuf::from(r"C:\Program Files\AtlasAgent")
    } else if cfg!(target_os = "macos") {
        PathBuf::from("/Library/Application Support/AtlasAgent")
    } else {
        PathBuf::from("/etc/atlas-agent")
    }
}
pub fn managed_drop_in_dir(root: &Path) -> PathBuf {
    root.join("managed-settings.d")
}
// src/utils/settings/schemaOutput.ts
pub fn settings_json_schema() -> String {
    serde_json::to_string_pretty(&json!({
        "$schema":"https://json-schema.org/draft/2020-12/schema",
        "type":"object",
        "properties":{
            "default_provider":{"type":"string"},
            "default_shell":{"type":["string","null"],"enum":["bash","powershell",null]},
            "providers":{"type":"array","items":{"type":"object",
                "required":["id","kind","model","base_url"],
                "properties":{"id":{"type":"string"},"kind":{"type":"string"},
                "model":{"type":"string"},"base_url":{"type":"string"}}}},
            "additional_directories":{"type":"array","items":{"type":"string"}},
            "mcp_servers":{"type":"array"},
            "hooks":{"type":"array"},
            "enabled_plugins":{"type":"array","items":{"type":"string"}}
        }
    }))
    .unwrap()
}
// src/utils/shell/shellToolUtils.ts
pub const SHELL_TOOL_NAMES: &[&str] = &["Bash", "PowerShell"];
pub fn powershell_enabled() -> bool {
    cfg!(windows)
}
pub fn is_shell_tool(name: &str) -> bool {
    SHELL_TOOL_NAMES.contains(&name) || name == "run_command"
}
// src/utils/standaloneAgent.ts
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StandaloneAgentContext {
    pub name: Option<String>,
    pub color: Option<String>,
}
pub fn standalone_agent_name<'a>(
    standalone: &'a StandaloneAgentContext,
    team: Option<&str>,
) -> Option<&'a str> {
    if team.is_some_and(|name| !name.is_empty()) {
        None
    } else {
        standalone.name.as_deref()
    }
}
pub fn standalone_agent_color<'a>(
    standalone: &'a StandaloneAgentContext,
    team: Option<&str>,
) -> Option<&'a str> {
    if team.is_some_and(|name| !name.is_empty()) {
        None
    } else {
        standalone.color.as_deref()
    }
}
// src/utils/swarm/teammatePromptAddendum.ts
pub const TEAMMATE_SYSTEM_PROMPT_ADDENDUM: &str = "# Agent Teammate Communication\n\
     This process is a teammate in a coordinated agent team.\n\
     Use SendMessage with to: \"agent-name\" for private messages or to: \"*\" for broadcasts.\n\
     Plain text responses are not visible to other teammates.\n\
     Coordinate tasks and notify the team lead via the task and message tools.";
// src/utils/todo/types.ts
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}
impl TodoStatus {
    pub fn parse(label: &str) -> Option<Self> {
        match label {
            "pending" => Some(Self::Pending),
            "in_progress" => Some(Self::InProgress),
            "completed" => Some(Self::Completed),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TodoItem {
    pub content: String,
    pub status: TodoStatus,
    pub active_form: String,
}
pub fn parse_todo_list(value: &Value) -> Result<Vec<TodoItem>> {
    let array = value
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Todo list must be an array"))?;
    if array.len() > 1000 {
        bail!("Todo list exceeds limit")
    }
    array
        .iter()
        .map(|v| {
            let content = v
                .get("content")
                .and_then(Value::as_str)
                .filter(|s| !s.trim().is_empty())
                .ok_or_else(|| anyhow::anyhow!("Missing todo content"))?;
            let status = v
                .get("status")
                .and_then(Value::as_str)
                .and_then(TodoStatus::parse)
                .ok_or_else(|| anyhow::anyhow!("Invalid todo status"))?;
            let active_form = v
                .get("activeForm")
                .and_then(Value::as_str)
                .unwrap_or(content)
                .to_owned();
            Ok(TodoItem {
                content: content.into(),
                status,
                active_form,
            })
        })
        .collect()
}
// src/utils/toolSchemaCache.ts
#[derive(Default)]
pub struct ToolSchemaCache {
    values: Mutex<HashMap<String, Value>>,
}
impl ToolSchemaCache {
    pub fn get(&self, key: &str) -> Option<Value> {
        self.values.lock().unwrap().get(key).cloned()
    }
    pub fn set(&self, key: &str, value: Value) {
        self.values.lock().unwrap().insert(key.into(), value);
    }
    pub fn clear(&self) {
        self.values.lock().unwrap().clear()
    }
    pub fn len(&self) -> usize {
        self.values.lock().unwrap().len()
    }
}
// src/utils/yaml.ts — JSON (YAML 1.2 subset) and flat YAML mapping parsing.
pub fn parse_yaml(input: &str) -> Result<Value> {
    if input.trim().is_empty() {
        return Ok(Value::Null);
    }
    if let Ok(value) = serde_json::from_str(input) {
        return Ok(value);
    }
    let mut result = serde_json::Map::new();
    let mut array = Vec::new();
    let mut list_mode = false;
    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || matches!(line, "---" | "...") {
            continue;
        }
        if let Some(rest) = line.strip_prefix("- ") {
            if !result.is_empty() {
                bail!("Cannot combine mapping and list at root")
            }
            list_mode = true;
            array.push(yaml_scalar(rest.trim()));
            continue;
        }
        if list_mode {
            bail!("Unexpected mapping after root list")
        }
        let (key, value) = line
            .split_once(':')
            .ok_or_else(|| anyhow::anyhow!("Invalid YAML key-value pair"))?;
        if key.trim().is_empty() {
            bail!("Empty YAML key")
        }
        result.insert(key.trim().to_owned(), yaml_scalar(value.trim()));
    }
    if list_mode {
        Ok(Value::Array(array))
    } else {
        Ok(Value::Object(result))
    }
}
fn yaml_scalar(text: &str) -> Value {
    let text = text.trim();
    if let Ok(v) = serde_json::from_str(text) {
        return v;
    }
    if matches!(text, "null" | "~") {
        return Value::Null;
    }
    if text.eq_ignore_ascii_case("true") {
        return json!(true);
    }
    if text.eq_ignore_ascii_case("false") {
        return json!(false);
    }
    Value::String(text.trim_matches(|c| c == '\'' || c == '"').into())
}
// src/utils/zodToJsonSchema.ts: JSON Schema conversion cache for native shapes.
#[derive(Default)]
pub struct JsonSchemaCache {
    shapes: Mutex<HashMap<String, Value>>,
}
impl JsonSchemaCache {
    pub fn convert(&self, identifier: &str, schema: &Value) -> Result<Value> {
        if let Some(value) = self.shapes.lock().unwrap().get(identifier).cloned() {
            return Ok(value);
        }
        if !schema.is_object() && !schema.is_boolean() {
            bail!("JSON Schema must be an object or boolean")
        }
        let value = schema.clone();
        self.shapes
            .lock()
            .unwrap()
            .insert(identifier.into(), value.clone());
        Ok(value)
    }
    pub fn clear(&self) {
        self.shapes.lock().unwrap().clear()
    }
}
// src/utils/classifierApprovalsHook.ts
#[derive(Default)]
pub struct ClassifierChecking {
    pending: Mutex<HashSet<String>>,
}
impl ClassifierChecking {
    pub fn set(&self, tool_id: &str, active: bool) {
        let mut state = self.pending.lock().unwrap();
        if active {
            state.insert(tool_id.into());
        } else {
            state.remove(tool_id);
        }
    }
    pub fn is_checking(&self, tool_id: &str) -> bool {
        self.pending.lock().unwrap().contains(tool_id)
    }
}
// src/utils/computerUse/inputLoader.ts
pub trait ComputerInput: Send + Sync {
    fn is_supported(&self) -> bool;
    fn send_key(&self, key: &str) -> Result<()>;
    fn send_text(&self, text: &str) -> Result<()>;
}
pub fn require_computer_input(
    configured: Option<&dyn ComputerInput>,
) -> Result<&dyn ComputerInput> {
    let input = configured.ok_or_else(|| anyhow::anyhow!("Computer input backend unavailable"))?;
    if !input.is_supported() {
        bail!("Computer input backend unsupported on this platform")
    }
    Ok(input)
}
// src/utils/plugins/gitAvailability.ts
#[derive(Default)]
pub struct GitAvailability {
    cached: Mutex<Option<bool>>,
}
impl GitAvailability {
    pub fn available(&self) -> bool {
        let mut state = self.cached.lock().unwrap();
        *state.get_or_insert_with(|| crate::shell_runtime::which("git").is_some())
    }
    pub fn mark_unavailable(&self) {
        *self.cached.lock().unwrap() = Some(false)
    }
    pub fn clear(&self) {
        *self.cached.lock().unwrap() = None
    }
}
// src/utils/settings/validateEditTool.ts — validate settings JSON only on edit.
#[derive(Clone, Debug)]
pub struct SettingsValidationError {
    pub code: u32,
    pub message: String,
}
pub fn validate_settings_edit(
    path: &Path,
    original: &str,
    updated: impl FnOnce() -> String,
) -> Option<SettingsValidationError> {
    if path.file_name().and_then(|n| n.to_str()) != Some("settings.json") {
        return None;
    }
    // Invalid source files must remain editable for repair.
    if serde_json::from_str::<Value>(original).is_err() {
        return None;
    }
    let new_content = updated();
    match serde_json::from_str::<Value>(&new_content) {
        Ok(value) if value.is_object() => None,
        Ok(_) => Some(SettingsValidationError {
            code: 10,
            message: "Settings root must be an object".into(),
        }),
        Err(error) => Some(SettingsValidationError {
            code: 10,
            message: format!("Invalid settings after edit: {error}"),
        }),
    }
}
// src/utils/bash/specs/nohup.ts and src/utils/bash/specs/sleep.ts
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShellCommandSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub arg_name: &'static str,
    pub required: bool,
    pub arg_is_command: bool,
}
pub fn nohup_spec() -> ShellCommandSpec {
    ShellCommandSpec {
        name: "nohup",
        description: "Run a command immune to hangups",
        arg_name: "command",
        required: true,
        arg_is_command: true,
    }
}
pub fn sleep_spec() -> ShellCommandSpec {
    ShellCommandSpec {
        name: "sleep",
        description: "Delay for a specified amount of time",
        arg_name: "duration",
        required: true,
        arg_is_command: false,
    }
}
pub fn parse_sleep_duration_ms(raw: &str) -> Result<u64> {
    let text = raw.trim();
    let end = text
        .find(|ch: char| !(ch.is_ascii_digit() || ch == '.'))
        .unwrap_or(text.len());
    let duration = text[..end].parse::<f64>()?;
    if !duration.is_finite() || duration < 0. {
        bail!("Invalid sleep duration")
    }
    let scale = match text[end..].trim() {
        "" | "s" => 1_000.,
        "m" => 60_000.,
        "h" => 3_600_000.,
        "d" => 86_400_000.,
        _ => bail!("Unsupported sleep duration unit"),
    };
    let millis = duration * scale;
    if millis > u64::MAX as f64 {
        bail!("Sleep duration too large")
    }
    Ok(millis as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_and_internal_writes() {
        let marks = InternalWriteMarks::default();
        marks.mark(Path::new("/tmp/config"));
        assert!(marks.consume(Path::new("/tmp/config"), Duration::from_secs(2)));
        assert!(!marks.consume(Path::new("/tmp/config"), Duration::from_secs(2)));
        let scopes = HashMap::from([("project".into(), vec!["bad MCP".into()])]);
        let result =
            merge_settings_and_mcp_errors(json!({"x":1}), vec!["bad setting".into()], &scopes);
        assert_eq!(result.errors, vec!["bad setting", "bad MCP"]);
        assert!(
            validate_settings_edit(Path::new("settings.json"), "{}", || "{broken".into()).is_some()
        );
        assert!(
            validate_settings_edit(Path::new("normal.json"), "{}", || "{broken".into()).is_none()
        );
        let cache = ToolSchemaCache::default();
        cache.set("read", json!({"name":"Read"}));
        assert_eq!(cache.get("read").unwrap()["name"], "Read");
        cache.clear();
        assert_eq!(cache.len(), 0);
    }
    #[test]
    fn todo_yaml_sleep_and_schema() {
        assert_eq!(
            parse_todo_list(&json!([{"content":"code","status":"pending","activeForm":"coding"}]))
                .unwrap()[0]
                .active_form,
            "coding"
        );
        assert_eq!(
            parse_yaml("name: atlas\ncount: 2\nactive: true").unwrap(),
            json!({"name":"atlas","count":2,"active":true})
        );
        assert_eq!(parse_yaml("- one\n- two").unwrap(), json!(["one", "two"]));
        assert_eq!(parse_sleep_duration_ms("2m").unwrap(), 120_000);
        assert_eq!(sleep_spec().name, "sleep");
        assert!(nohup_spec().arg_is_command);
        assert!(settings_json_schema().contains("default_provider"));
    }
}
