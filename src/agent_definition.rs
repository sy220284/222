//! User/project agent definition validation and durable Markdown storage.
//! Implements the general-purpose agent editor's format, validation and file
//! lifecycle without requiring provider-specific credentials or telemetry.
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDefinition {
    pub agent_type: String,
    pub when_to_use: String,
    pub system_prompt: String,
    pub tools: Option<Vec<String>>,
    pub color: Option<String>,
    pub model: Option<String>,
    pub memory: Option<String>,
    pub effort: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Validation {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl Validation {
    pub fn valid(&self) -> bool {
        self.errors.is_empty()
    }
}

pub fn validate_agent_type(name: &str) -> Option<&'static str> {
    if name.is_empty() {
        return Some("Agent type is required");
    }
    if !name
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric())
        || !name
            .chars()
            .last()
            .is_some_and(|c| c.is_ascii_alphanumeric())
        || name
            .bytes()
            .any(|c| !c.is_ascii_alphanumeric() && c != b'-')
    {
        return Some("Agent type must start and end with alphanumeric characters and contain only letters, numbers, and hyphens");
    }
    if name.len() < 3 {
        return Some("Agent type must be at least 3 characters long");
    }
    if name.len() > 50 {
        return Some("Agent type must be less than 50 characters");
    }
    None
}

pub fn validate(
    agent: &AgentDefinition,
    known_tools: &HashSet<String>,
    duplicate_sources: &[String],
) -> Validation {
    let mut result = Validation::default();
    if let Some(error) = validate_agent_type(&agent.agent_type) {
        result.errors.push(error.into());
    }
    for other in duplicate_sources {
        result.errors.push(format!(
            "Agent type \"{}\" already exists in {other}",
            agent.agent_type
        ));
    }
    if agent.when_to_use.is_empty() {
        result
            .errors
            .push("Description (description) is required".into());
    } else if agent.when_to_use.len() < 10 {
        result
            .warnings
            .push("Description should be more descriptive (at least 10 characters)".into());
    } else if agent.when_to_use.len() > 5000 {
        result
            .warnings
            .push("Description is very long (over 5000 characters)".into());
    }
    match &agent.tools {
        None => result.warnings.push("Agent has access to all tools".into()),
        Some(tools) if tools.is_empty() => result
            .warnings
            .push("No tools selected - agent will have very limited capabilities".into()),
        Some(tools) => {
            let unknown: Vec<_> = tools
                .iter()
                .filter(|name| name.as_str() != "*" && !known_tools.contains(*name))
                .cloned()
                .collect();
            if !unknown.is_empty() {
                result
                    .errors
                    .push(format!("Invalid tools: {}", unknown.join(", ")));
            }
        }
    }
    if agent.system_prompt.is_empty() {
        result.errors.push("System prompt is required".into());
    } else if agent.system_prompt.len() < 20 {
        result
            .errors
            .push("System prompt is too short (minimum 20 characters)".into());
    } else if agent.system_prompt.len() > 10000 {
        result
            .warnings
            .push("System prompt is very long (over 10,000 characters)".into());
    }
    result
}

pub fn format_as_markdown(agent: &AgentDefinition) -> String {
    let escaped_description = agent
        .when_to_use
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\\\n");
    let mut out = format!(
        "---\nname: {}\ndescription: \"{}\"",
        agent.agent_type, escaped_description
    );
    if let Some(tools) = agent
        .tools
        .as_ref()
        .filter(|tools| tools.as_slice() != ["*"])
    {
        out.push_str(&format!("\ntools: {}", tools.join(", ")));
    }
    for (key, value) in [
        ("model", &agent.model),
        ("effort", &agent.effort),
        ("color", &agent.color),
        ("memory", &agent.memory),
    ] {
        if let Some(value) = value.as_ref().filter(|v| !v.is_empty()) {
            out.push_str(&format!("\n{key}: {value}"));
        }
    }
    out.push_str("\n---\n\n");
    out.push_str(&agent.system_prompt);
    out.push('\n');
    out
}

pub fn directory(workspace: &Path, user: bool) -> Result<PathBuf> {
    if user {
        let home = std::env::var_os("ATLAS_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|home| home.join(".atlas")))
            .context("Unable to identify the user config directory")?;
        Ok(home.join("agents"))
    } else {
        Ok(workspace.join(".atlas").join("agents"))
    }
}

pub fn new_file_path(workspace: &Path, user: bool, agent_type: &str) -> Result<PathBuf> {
    if let Some(error) = validate_agent_type(agent_type) {
        bail!("{error}");
    }
    Ok(directory(workspace, user)?.join(format!("{agent_type}.md")))
}

pub fn existing_agent_types(dir: &Path) -> Result<HashSet<String>> {
    let mut result = HashSet::new();
    let entries = match fs::read_dir(dir) {
        Ok(it) => it,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(result),
        Err(e) => return Err(e).with_context(|| format!("Cannot read {}", dir.display())),
    };
    for file in entries {
        let file = file?;
        if file.file_type()?.is_file() {
            if let Some(name) = file.path().file_stem().and_then(|s| s.to_str()) {
                if file.path().extension().is_some_and(|ext| ext == "md") {
                    result.insert(name.to_owned());
                }
            }
        }
    }
    Ok(result)
}

pub fn validate_in_workspace(
    agent: &AgentDefinition,
    workspace: &Path,
    user: bool,
) -> Result<Validation> {
    let known_tools = crate::tools::definitions()
        .into_iter()
        .filter_map(|tool| {
            tool.get("function")
                .and_then(|f| f.get("name"))
                .or_else(|| tool.get("name"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .collect();
    let duplicates = if user {
        let mut existing = existing_agent_types(&directory(workspace, false)?)?;
        if existing.remove(&agent.agent_type) {
            vec!["project settings".to_owned()]
        } else {
            Vec::new()
        }
    } else if existing_agent_types(&directory(workspace, true)?)?.contains(&agent.agent_type) {
        vec!["user settings".to_owned()]
    } else {
        Vec::new()
    };
    Ok(validate(agent, &known_tools, &duplicates))
}

pub fn save_agent(
    agent: &AgentDefinition,
    workspace: &Path,
    user: bool,
    replace: bool,
) -> Result<PathBuf> {
    let validation = validate_in_workspace(agent, workspace, user)?;
    if !validation.valid() {
        bail!("{}", validation.errors.join("; "));
    }
    let path = new_file_path(workspace, user, &agent.agent_type)?;
    fs::create_dir_all(path.parent().context("Invalid agent directory")?)?;
    // Never follow a symlink on replacement: the original editor requires
    // the selected agent file, not arbitrary redirection to other files.
    if replace && fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
        bail!("Cannot replace a symbolic link: {}", path.display());
    }
    let mut options = OpenOptions::new();
    options.write(true);
    if replace {
        options.create(true).truncate(true);
    } else {
        options.create_new(true);
    }
    let mut file = options
        .open(&path)
        .with_context(|| format!("Cannot write agent file {}", path.display()))?;
    file.write_all(format_as_markdown(agent).as_bytes())?;
    file.sync_data()?;
    Ok(path)
}

pub fn delete_agent(workspace: &Path, user: bool, agent_type: &str) -> Result<()> {
    let path = new_file_path(workspace, user, agent_type)?;
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> AgentDefinition {
        AgentDefinition {
            agent_type: "review-agent".into(),
            when_to_use: "Review recent code changes".into(),
            system_prompt: "You review changes carefully and report actionable issues.".into(),
            tools: Some(vec!["read_file".into()]),
            color: None,
            model: Some("local-code-model".into()),
            memory: None,
            effort: None,
        }
    }
    #[test]
    fn validation_catches_name_description_prompt_and_tool_errors() {
        assert!(validate_agent_type("a").is_some());
        assert!(validate_agent_type("bad/path").is_some());
        assert!(validate_agent_type("invalid-").is_some());
        assert_eq!(validate_agent_type("agent-review"), None);
        let mut a = sample();
        a.when_to_use.clear();
        a.tools = Some(vec!["Nonexistent".into()]);
        a.system_prompt = "short".into();
        let v = validate(
            &a,
            &HashSet::from(["read_file".into()]),
            &["project".into()],
        );
        assert_eq!(v.errors.len(), 4);
    }
    #[test]
    fn markdown_escape_and_optional_fields_match_source_format() {
        let mut a = sample();
        a.when_to_use = "line\\one\nline\"two".into();
        let text = format_as_markdown(&a);
        assert!(text
            .starts_with("---\nname: review-agent\ndescription: \"line\\\\one\\\\nline\\\"two\""));
        assert!(text.contains("\ntools: read_file\nmodel: local-code-model\n---\n\n"));
        a.tools = Some(vec!["*".into()]);
        assert!(!format_as_markdown(&a).contains("tools:"));
    }
    #[test]
    fn create_update_delete_persists_file_and_blocks_cross_source_duplicates() {
        let dir = tempfile::tempdir().unwrap();
        let mut a = sample();
        let workspace = dir.path();
        let path = save_agent(&a, workspace, false, false).unwrap();
        assert!(path.exists());
        assert!(save_agent(&a, workspace, false, false).is_err());
        a.system_prompt.push_str(" Another statement.");
        save_agent(&a, workspace, false, true).unwrap();
        assert!(fs::read_to_string(&path)
            .unwrap()
            .contains("Another statement."));
        delete_agent(workspace, false, "review-agent").unwrap();
        assert!(!path.exists());
    }
}
