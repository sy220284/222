use crate::{
    cost::ModelPrice,
    hooks::{HookRule, HookRunner},
};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderProfile {
    pub id: String,
    /// openai-compatible or gemini
    pub kind: String,
    pub base_url: String,
    pub model: String,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub stream: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub id: String,
    #[serde(default)]
    pub command: String,
    #[serde(default, rename = "type")]
    pub transport: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
    /// Shell command producing a JSON object of per-connection HTTP headers.
    #[serde(default, rename = "headersHelper", alias = "headers_helper")]
    pub headers_helper: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspServerConfig {
    pub id: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub extensions: Vec<String>,
    #[serde(default)]
    pub language_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchBackend {
    pub kind: String,
    pub base_url: String,
    #[serde(default)]
    pub api_key_env: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub default_provider: String,
    pub providers: Vec<ProviderProfile>,
    #[serde(default)]
    pub additional_directories: Vec<String>,
    /// Glob patterns for project/user/local instruction files omitted from model context.
    #[serde(default, rename = "instructionExcludes", alias = "instruction_excludes")]
    pub instruction_excludes: Vec<String>,
    /// Per-source instruction switches: user, project and local (default enabled).
    #[serde(default, rename = "instructionSources", alias = "instruction_sources")]
    pub instruction_sources: std::collections::HashMap<String, bool>,
    /// Explicit user approval for instruction @includes outside the workspace.
    #[serde(default, rename = "approvedExternalInstructionIncludes", alias = "approved_external_instruction_includes")]
    pub approved_external_instruction_includes: bool,
    /// Administrator-provided global instructions, independent of user/project switches.
    #[serde(default, rename = "managedInstructionFile", alias = "managed_instruction_file")]
    pub managed_instruction_file: Option<String>,
    #[serde(default, rename = "managedRulesDirectory", alias = "managed_rules_directory")]
    pub managed_rules_directory: Option<String>,
    #[serde(default)]
    pub default_shell: Option<String>,
    /// Preferred terminal for browser deep links, retained across headless launches.
    #[serde(
        default,
        rename = "deepLinkTerminal",
        alias = "deep_link_terminal",
        skip_serializing_if = "Option::is_none"
    )]
    pub deep_link_terminal: Option<String>,
    #[serde(default)]
    pub mcp_servers: Vec<McpServerConfig>,
    #[serde(default)]
    pub hooks: Vec<HookRule>,
    #[serde(default)]
    pub lsp_servers: Vec<LspServerConfig>,
    #[serde(default)]
    pub enabled_plugins: Vec<String>,
    #[serde(default)]
    pub github_repo_paths: std::collections::HashMap<String, Vec<String>>,
    #[serde(default)]
    pub search_backend: Option<SearchBackend>,
    #[serde(default)]
    pub permission_rules: Vec<crate::permission_rules::RuleSetting>,
    #[serde(default)]
    pub model_prices: Vec<ModelPrice>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_provider: "local".into(),
            additional_directories: Vec::new(),
            instruction_excludes: Vec::new(),
            instruction_sources: std::collections::HashMap::new(),
            approved_external_instruction_includes: false,
            managed_instruction_file: None,
            managed_rules_directory: None,
            default_shell: None,
            deep_link_terminal: None,
            providers: vec![
                ProviderProfile {
                    id: "local".into(),
                    kind: "openai-compatible".into(),
                    base_url: "http://127.0.0.1:11434/v1".into(),
                    model: "qwen2.5-coder:7b".into(),
                    api_key_env: None,
                    stream: false,
                },
                ProviderProfile {
                    id: "openai".into(),
                    kind: "openai-compatible".into(),
                    base_url: "https://api.openai.com/v1".into(),
                    model: "gpt-4.1-mini".into(),
                    api_key_env: Some("OPENAI_API_KEY".into()),
                    stream: false,
                },
                ProviderProfile {
                    id: "gemini".into(),
                    kind: "gemini".into(),
                    base_url: "https://generativelanguage.googleapis.com/v1beta".into(),
                    model: "gemini-2.5-flash".into(),
                    api_key_env: Some("GEMINI_API_KEY".into()),
                    stream: false,
                },
            ],
            mcp_servers: vec![],
            hooks: vec![],
            lsp_servers: vec![],
            enabled_plugins: vec![],
            github_repo_paths: std::collections::HashMap::new(),
            search_backend: None,
            permission_rules: vec![],
            model_prices: vec![],
        }
    }
}

impl Config {
    pub fn path() -> Result<PathBuf> {
        if let Ok(p) = std::env::var("ATLAS_CONFIG") {
            return Ok(PathBuf::from(p));
        }
        let home =
            dirs::config_dir().context("Cannot locate user config directory; set ATLAS_CONFIG")?;
        Ok(home.join("atlas-agent").join("config.json"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(&path).with_context(|| format!("Read {}", path.display()))?;
        let config: Self = serde_json::from_str(crate::json_read::strip_bom(&raw))
            .with_context(|| format!("Parse {}", path.display()))?;
        config.validate()?;
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let temp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        let bytes = serde_json::to_vec_pretty(self)?;
        let result = (|| -> Result<()> {
            use std::io::Write;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            fs::rename(&temp, &path)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(temp);
        }
        result
    }
    pub fn init() -> Result<PathBuf> {
        let path = Self::path()?;
        if path.exists() {
            bail!("Config already exists at {}", path.display());
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(&Self::default())?;
        fs::write(&path, format!("{data}\n"))?;
        Ok(path)
    }

    fn validate(&self) -> Result<()> {
        crate::shell_runtime::from_config(self.default_shell.as_deref())?;
        if self.providers.is_empty() {
            bail!("At least one provider is required")
        }
        let mut seen = std::collections::HashSet::new();
        for p in &self.providers {
            if p.id.trim().is_empty() || p.model.trim().is_empty() {
                bail!("Provider id and model must be nonempty");
            }
            if !seen.insert(&p.id) {
                bail!("Duplicate provider id: {}", p.id);
            }
            if !matches!(p.kind.as_str(), "openai-compatible" | "gemini") {
                bail!("Unsupported provider kind for {}: {}", p.id, p.kind);
            }
            let url = reqwest::Url::parse(&p.base_url)?;
            if !matches!(url.scheme(), "http" | "https") {
                bail!("Only http(s) endpoints are supported")
            }
        }
        let mut server_ids = std::collections::HashSet::new();
        for s in &self.mcp_servers {
            if s.id.is_empty()
                || s.id.len() > 24
                || !s
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
            {
                bail!("Invalid MCP server id: {}", s.id);
            }
            if !server_ids.insert(&s.id) {
                bail!("Duplicate MCP server id: {}", s.id);
            }
            match s.transport.as_deref().unwrap_or("stdio") {
                "stdio" => {
                    if s.command.trim().is_empty() {
                        bail!("MCP stdio server command cannot be empty")
                    }
                    if s.url.is_some()
                        || !s.headers.is_empty()
                        || s.headers_helper
                            .as_deref()
                            .is_some_and(|v| !v.trim().is_empty())
                    {
                        bail!("MCP stdio server cannot define HTTP url/headers/helper");
                    }
                }
                "http" => {
                    if !s.command.is_empty() || !s.args.is_empty() || !s.env.is_empty() {
                        bail!("MCP HTTP server cannot define stdio command, args or env");
                    }
                    let url = s.url.as_deref().context("MCP HTTP server requires url")?;
                    let expanded = crate::compat_helpers::expand_env_vars_in_string(url);
                    if !expanded.missing_vars.is_empty() {
                        bail!(
                            "MCP {} missing URL environment variables: {}",
                            s.id,
                            expanded.missing_vars.join(", ")
                        );
                    }
                    let parsed = reqwest::Url::parse(&expanded.expanded)?;
                    if !matches!(parsed.scheme(), "http" | "https")
                        || !parsed.username().is_empty()
                        || parsed.password().is_some()
                    {
                        bail!("MCP HTTP server requires http(s) URL without credentials");
                    }
                }
                _ => bail!("Unsupported MCP transport; use stdio or http"),
            }
        }
        if let Some(search) = &self.search_backend {
            if !matches!(search.kind.as_str(), "searxng" | "brave") {
                bail!("Unsupported search backend kind {}", search.kind);
            }
            let url = reqwest::Url::parse(&search.base_url)?;
            if !matches!(url.scheme(), "http" | "https") {
                bail!("Search endpoint requires http(s)");
            }
        }
        let mut rate_keys = std::collections::HashSet::new();
        for price in &self.model_prices {
            price.validate()?;
            if !seen.contains(&price.provider) {
                bail!("Price refers to unknown provider {}", price.provider);
            }
            if !rate_keys.insert((price.provider.as_str(), price.model.as_str())) {
                bail!(
                    "Duplicate model price for {}/{}",
                    price.provider,
                    price.model
                );
            }
        }
        HookRunner::new(&self.hooks)?;
        for server in &self.lsp_servers {
            if server.id.trim().is_empty()
                || server.command.trim().is_empty()
                || server.extensions.is_empty()
            {
                bail!("LSP server must define id, command, and extensions")
            }
        }
        if !seen.contains(&self.default_provider) {
            bail!("Default provider is missing");
        }
        Ok(())
    }

    pub fn select(&self, selected: Option<&str>, model: Option<&str>) -> Result<ProviderProfile> {
        let id = selected.unwrap_or(&self.default_provider);
        let mut p = self
            .providers
            .iter()
            .find(|p| p.id == id)
            .with_context(|| format!("No provider named {id}; run 'atlas-agent models'"))?
            .clone();
        if let Some(m) = model {
            if m.trim().is_empty() {
                bail!("Model cannot be empty");
            }
            p.model = m.to_owned();
        }
        Ok(p)
    }
}

pub fn canonical_workspace(path: &Path) -> Result<PathBuf> {
    let root = path
        .canonicalize()
        .with_context(|| format!("Workspace {} unavailable", path.display()))?;
    if !root.is_dir() {
        bail!("Workspace must be a directory")
    }
    Ok(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn config_with_utf8_bom_is_parsed_without_loose_json_coercion() {
        let raw = serde_json::to_string(&Config::default()).unwrap();
        let value = format!("{}{}", '\u{feff}', raw);
        let parsed: Config = serde_json::from_str(crate::json_read::strip_bom(&value)).unwrap();
        parsed.validate().unwrap();
        let invalid = format!(" {}{}", '\u{feff}', raw);
        assert!(serde_json::from_str::<Config>(crate::json_read::strip_bom(&invalid)).is_err());
    }
    #[test]
    fn validates_mcp_transport_variants_and_secret_avoidance() {
        let mut config = Config::default();
        config.mcp_servers.push(McpServerConfig {
            id: "remote".into(),
            transport: Some("http".into()),
            command: String::new(),
            args: Vec::new(),
            env: std::collections::HashMap::new(),
            url: Some("http://127.0.0.1:9876/mcp".into()),
            headers: std::collections::HashMap::new(),
            headers_helper: None,
        });
        assert!(config.validate().is_ok());
        config.mcp_servers[0].command = "echo".into();
        assert!(config.validate().is_err());
        config.mcp_servers[0].command.clear();
        config.mcp_servers[0].url = Some("http://user:secret@example.com/mcp".into());
        assert!(config.validate().is_err());
        config.mcp_servers[0].url = Some("http://127.0.0.1:9876/mcp".into());
        config.mcp_servers[0].transport = Some("unknown".into());
        assert!(config.validate().is_err());
    }
    #[test]
    fn profiles_have_unique_names() {
        Config::default().validate().unwrap();
    }
    #[test]
    fn selecting_custom_model_does_not_mutate_config() {
        let c = Config::default();
        assert_eq!(
            c.select(Some("local"), Some("example")).unwrap().model,
            "example"
        );
        assert_eq!(
            c.select(Some("local"), None).unwrap().model,
            "qwen2.5-coder:7b"
        );
    }
}
