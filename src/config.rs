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

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ModelTuning {
    /// Sampling settings are omitted unless configured; thinking models often reject them.
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub max_output_tokens: Option<u32>,
    /// A client-side budget hint, never sent as an unrecognized API argument.
    pub context_window: Option<u32>,
    pub reasoning_effort: Option<String>,
}

impl ModelTuning {
    pub fn validate(&self) -> Result<()> {
        if self
            .temperature
            .is_some_and(|v| !v.is_finite() || !(0.0..=2.0).contains(&v))
        {
            bail!("temperature must be finite and between 0 and 2");
        }
        if self
            .top_p
            .is_some_and(|v| !v.is_finite() || v <= 0.0 || v > 1.0)
        {
            bail!("top_p must be finite and within (0, 1]");
        }
        if self.max_output_tokens == Some(0) || self.context_window == Some(0) {
            bail!("max_output_tokens and context_window must be positive");
        }
        if let (Some(max), Some(context)) = (self.max_output_tokens, self.context_window) {
            if max > context {
                bail!("max_output_tokens cannot exceed context_window");
            }
        }
        if let Some(level) = &self.reasoning_effort {
            if !matches!(
                level.as_str(),
                "none" | "low" | "medium" | "high" | "xhigh" | "max"
            ) {
                bail!("reasoning_effort must be none, low, medium, high, xhigh or max");
            }
        }
        Ok(())
    }

    /// Check effective API behavior, including provider-specific ignored settings.
    pub fn validate_for_model(&self, model: &str) -> Result<()> {
        self.validate()?;
        let effort = self.reasoning_effort.as_deref();
        if model.starts_with("deepseek-") {
            let thinking = effort != Some("none");
            if thinking && self.temperature.is_some() {
                bail!("DeepSeek thinking mode ignores temperature; choose reasoning_effort=none before adjusting temperature");
            }
            if model == "deepseek-flash" {
                if thinking && self.top_p.is_some_and(|value| value < 0.95) {
                    bail!("DeepSeek Flash thinking mode requires top_p >= 0.95");
                }
                if !thinking && self.top_p.is_some() {
                    bail!("DeepSeek Flash non-thinking mode ignores top_p");
                }
            }
            if model == "deepseek-v4-pro" && thinking && self.top_p.is_some() {
                bail!("DeepSeek V4 Pro thinking mode ignores top_p");
            }
        }
        if matches!(
            model,
            "kimi-k3.1-pro" | "kimi-k3.1-pro-highspeed" | "k3.1-pro" | "k3.1-pro-highspeed"
        ) && effort == Some("none")
        {
            bail!("Kimi K3.1 Pro only supports thinking mode");
        }
        if model == "gpt-6.1-sol" && effort == Some("none") {
            bail!("GPT-6.1 Sol does not support reasoning_effort=none");
        }
        Ok(())
    }

    pub fn apply_chat_completions(&self, body: &mut serde_json::Value) {
        if let Some(v) = self.temperature {
            body["temperature"] = serde_json::json!(v);
        }
        if let Some(v) = self.top_p {
            body["top_p"] = serde_json::json!(v);
        }
        if let Some(v) = self.max_output_tokens {
            body["max_tokens"] = serde_json::json!(v);
        }
        if let Some(v) = &self.reasoning_effort {
            body["reasoning_effort"] = serde_json::json!(v);
        }
    }
}

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
    #[serde(default)]
    pub tuning: ModelTuning,
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
#[serde(default)]
pub struct Config {
    /// Original fileHistoryEnabled user setting, enabled for interactive sessions.
    #[serde(
        default = "default_file_checkpointing",
        rename = "fileCheckpointingEnabled",
        alias = "file_checkpointing_enabled"
    )]
    pub file_checkpointing_enabled: bool,
    pub default_provider: String,
    /// Only NODE_EXTRA_CA_CERTS is applied at process startup. Other keys
    /// remain stored user settings and do not automatically enter the environment.
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
    #[serde(
        default = "default_editor_mode",
        rename = "editorMode",
        alias = "editor_mode"
    )]
    pub editor_mode: String,
    /// User-selected reviewer provider, independent of the main-loop provider.
    #[serde(
        default,
        rename = "advisorModel",
        alias = "advisor_model",
        skip_serializing_if = "Option::is_none"
    )]
    pub advisor_model: Option<String>,
    pub providers: Vec<ProviderProfile>,
    #[serde(default)]
    pub additional_directories: Vec<String>,
    /// Glob patterns for project/user/local instruction files omitted from model context.
    #[serde(
        default,
        rename = "instructionExcludes",
        alias = "instruction_excludes"
    )]
    pub instruction_excludes: Vec<String>,
    /// Per-source instruction switches: user, project and local (default enabled).
    #[serde(default, rename = "instructionSources", alias = "instruction_sources")]
    pub instruction_sources: std::collections::HashMap<String, bool>,
    /// Per-project approval, keyed by canonical workspace path, for external @includes.
    #[serde(
        default,
        rename = "approvedExternalInstructionIncludes",
        alias = "approved_external_instruction_includes"
    )]
    pub approved_external_instruction_includes: std::collections::HashMap<String, bool>,
    /// Administrator-provided global instructions, independent of user/project switches.
    #[serde(
        default,
        rename = "managedInstructionFile",
        alias = "managed_instruction_file"
    )]
    pub managed_instruction_file: Option<String>,
    #[serde(
        default,
        rename = "managedRulesDirectory",
        alias = "managed_rules_directory"
    )]
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

fn default_editor_mode() -> String {
    "normal".into()
}

fn default_file_checkpointing() -> bool {
    true
}

/** Built-in Chinese model API profiles. Credentials are read from environment variables. */
fn chatgpt_plan_profile() -> ProviderProfile {
    ProviderProfile {
        id: "chatgpt".into(),
        kind: "chatgpt-plan".into(),
        base_url: "https://api.openai.com/v1".into(),
        model: "gpt-6.1-sol".into(),
        api_key_env: None,
        stream: true,
        tuning: ModelTuning::default(),
    }
}

fn domestic_api_profiles() -> Vec<ProviderProfile> {
    [
        (
            "deepseek",
            "https://api.deepseek.com",
            "deepseek-flash",
            "DEEPSEEK_API_KEY",
        ),
        (
            "kimi",
            "https://api.moonshot.cn/v1",
            "kimi-k3.1-pro",
            "MOONSHOT_API_KEY",
        ),
        (
            "glm",
            "https://open.bigmodel.cn/api/paas/v4",
            "glm-5.3",
            "ZHIPU_API_KEY",
        ),
        (
            "minimax",
            "https://api.minimaxi.com/v1",
            "MiniMax-M3",
            "MINIMAX_API_KEY",
        ),
    ]
    .into_iter()
    .map(|(id, base_url, model, api_key_env)| ProviderProfile {
        id: id.into(),
        kind: "openai-compatible".into(),
        base_url: base_url.into(),
        model: model.into(),
        api_key_env: Some(api_key_env.into()),
        stream: false,
        tuning: ModelTuning::default(),
    })
    .collect()
}

impl Default for Config {
    fn default() -> Self {
        let mut config = Self {
            default_provider: "openai".into(),
            env: std::collections::HashMap::new(),
            editor_mode: default_editor_mode(),
            advisor_model: None,
            file_checkpointing_enabled: true,
            additional_directories: Vec::new(),
            instruction_excludes: Vec::new(),
            instruction_sources: std::collections::HashMap::new(),
            approved_external_instruction_includes: std::collections::HashMap::new(),
            managed_instruction_file: None,
            managed_rules_directory: None,
            default_shell: None,
            deep_link_terminal: None,
            providers: vec![
                ProviderProfile {
                    id: "openai".into(),
                    kind: "openai-responses".into(),
                    base_url: "https://api.openai.com/v1".into(),
                    model: "gpt-6.1-sol".into(),
                    api_key_env: Some("OPENAI_API_KEY".into()),
                    stream: false,
                    tuning: ModelTuning::default(),
                },
                ProviderProfile {
                    id: "gemini".into(),
                    kind: "gemini".into(),
                    base_url: "https://generativelanguage.googleapis.com/v1beta".into(),
                    model: "gemini-3.8-flash".into(),
                    api_key_env: Some("GEMINI_API_KEY".into()),
                    stream: false,
                    tuning: ModelTuning::default(),
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
        };
        config.include_domestic_api_profiles();
        config
    }
}

impl Config {
    /// Append bundled APIs to old settings in memory; custom provider IDs win.
    /// The user's config file is not rewritten merely by reading it.
    fn include_domestic_api_profiles(&mut self) {
        for builtin in domestic_api_profiles()
            .into_iter()
            .chain(std::iter::once(chatgpt_plan_profile()))
        {
            if !self
                .providers
                .iter()
                .any(|existing| existing.id == builtin.id)
            {
                self.providers.push(builtin);
            }
        }
    }

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
        let mut config: Self = serde_json::from_str(crate::json_read::strip_bom(&raw))
            .with_context(|| format!("Parse {}", path.display()))?;
        // Preserve the old configuration and its custom overrides while making
        // the new API providers immediately selectable after an upgrade.
        if !config.providers.is_empty() {
            config.include_domestic_api_profiles();
        }
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

    pub(crate) fn validate(&self) -> Result<()> {
        crate::shell_runtime::from_config(self.default_shell.as_deref())?;
        if self.providers.is_empty() {
            bail!("At least one provider is required")
        }
        let mut seen = std::collections::HashSet::new();
        for p in &self.providers {
            p.tuning.validate_for_model(&p.model)?;
            if p.id.trim().is_empty() || p.model.trim().is_empty() {
                bail!("Provider id and model must be nonempty");
            }
            if !seen.insert(&p.id) {
                bail!("Duplicate provider id: {}", p.id);
            }
            if !matches!(
                p.kind.as_str(),
                "openai-compatible" | "openai-responses" | "gemini" | "chatgpt-plan"
            ) {
                bail!("Unsupported provider kind for {}: {}", p.id, p.kind);
            }
            let url = reqwest::Url::parse(&p.base_url)?;
            // Reject the removed Ollama default in persisted configurations.
            // Keep loopback endpoints available for API protocol integration tests.
            if matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1"))
                && url.port_or_known_default() == Some(11434)
            {
                bail!("The local model endpoint was removed; configure an API provider");
            }
            if !matches!(url.scheme(), "http" | "https") {
                bail!("Only http(s) endpoints are supported")
            }
            require_secure_credential_transport(&p.base_url, p.api_key_env.is_some())?;
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
                "http" | "sse" => {
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
                    require_secure_credential_transport(
                        &expanded.expanded,
                        s.headers_helper
                            .as_deref()
                            .is_some_and(|helper| !helper.trim().is_empty())
                            || s.headers
                                .iter()
                                .any(|(name, value)| credential_header(name, value)),
                    )?;
                }
                _ => bail!("Unsupported MCP transport; use stdio, http or sse"),
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
            require_secure_credential_transport(
                &search.base_url,
                search.kind == "brave" && search.api_key_env.is_some(),
            )?;
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

/// Never send an API credential over cleartext remote HTTP. Provider-neutral
/// loopback endpoints remain available for local runtimes and development.
/// Classify token-like request headers before any environment expansion so
/// user secrets cannot be sent to a non-loopback plaintext MCP endpoint.
pub(crate) fn credential_header(name: &str, value: &str) -> bool {
    let name = name.to_ascii_lowercase();
    // Environment interpolation may carry a secret even when the header name
    // is innocuous (e.g. X-Custom: ${API_TOKEN}). Classify before expansion
    // so the original secret never has to be logged or inspected.
    value.contains("${")
        || name == "authorization"
        || name == "proxy-authorization"
        || name == "cookie"
        || name == "set-cookie"
        || name.contains("token")
        || name.contains("secret")
        || name.contains("api-key")
        || name.ends_with("-key")
        || value
            .trim_start()
            .to_ascii_lowercase()
            .starts_with("bearer ")
        || value
            .trim_start()
            .to_ascii_lowercase()
            .starts_with("basic ")
}

pub(crate) fn require_secure_credential_transport(url: &str, has_credential: bool) -> Result<()> {
    let parsed = reqwest::Url::parse(url)?;
    // Credentials embedded in a URL are still credentials even if no
    // api_key_env is configured. A URL can carry Basic auth userinfo or
    // API keys as query parameters.
    let url_has_credential = !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query_pairs().any(|(key, _)| {
            let key = key.to_ascii_lowercase();
            matches!(
                key.as_str(),
                "key"
                    | "apikey"
                    | "api_key"
                    | "access_key"
                    | "access_token"
                    | "auth"
                    | "authorization"
                    | "client_secret"
                    | "password"
                    | "passwd"
            ) || credential_header(&key, "")
        });
    if (has_credential || url_has_credential)
        && parsed.scheme() == "http"
        && !matches!(parsed.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))
    {
        bail!("Remote endpoint carrying credentials must use HTTPS");
    }
    Ok(())
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
    fn credentialed_endpoints_require_tls_except_loopback() {
        assert!(credential_header("Authorization", "${API_TOKEN}"));
        assert!(credential_header("X-Subscription-Token", "secret"));
        assert!(credential_header("X-Custom", "Bearer token"));
        assert!(credential_header("X-Custom", "${MY_CREDENTIAL}"));
        assert!(!credential_header("X-Client-Version", "1.0"));
        for url in [
            "http://example.com/v1",
            "http://10.0.0.5:3000/v1",
            "http://192.168.1.2/v1",
        ] {
            assert!(
                require_secure_credential_transport(url, true).is_err(),
                "{url}"
            );
            assert!(
                require_secure_credential_transport(url, false).is_ok(),
                "{url}"
            );
        }
        for url in [
            "http://user:password@example.com/v1",
            "http://example.com/v1?api_key=example",
            "http://example.com/v1?access_token=example",
        ] {
            assert!(
                require_secure_credential_transport(url, false).is_err(),
                "embedded URL credential must require TLS: {url}"
            );
        }
        for url in [
            "http://localhost:11434/v1",
            "http://127.0.0.1:11434/v1",
            "http://[::1]:11434/v1",
            "https://example.com/v1",
        ] {
            assert!(
                require_secure_credential_transport(url, true).is_ok(),
                "{url}"
            );
        }
    }
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
    fn api_only_default_rejects_discontinued_local_model() {
        let mut c = Config::default();
        assert_eq!(c.default_provider, "openai");
        assert!(c.providers.iter().all(|p| !p.base_url.contains(":11434")));
        c.providers[0].base_url = "http://127.0.0.1:11434/v1".into();
        assert!(c.validate().is_err());
    }

    #[test]
    fn domestic_api_profiles_are_configured_with_official_endpoints() {
        let config = Config::default();
        config.validate().unwrap();
        assert_eq!(config.providers.len(), 7);
        for (id, url, model, env) in [
            (
                "deepseek",
                "https://api.deepseek.com",
                "deepseek-flash",
                "DEEPSEEK_API_KEY",
            ),
            (
                "kimi",
                "https://api.moonshot.cn/v1",
                "kimi-k3.1-pro",
                "MOONSHOT_API_KEY",
            ),
            (
                "glm",
                "https://open.bigmodel.cn/api/paas/v4",
                "glm-5.3",
                "ZHIPU_API_KEY",
            ),
            (
                "minimax",
                "https://api.minimaxi.com/v1",
                "MiniMax-M3",
                "MINIMAX_API_KEY",
            ),
        ] {
            let profile = config.select(Some(id), None).unwrap();
            assert_eq!(profile.kind, "openai-compatible");
            assert_eq!(profile.base_url, url);
            assert_eq!(profile.model, model);
            assert_eq!(profile.api_key_env.as_deref(), Some(env));
            assert!(!profile.stream);
            assert_eq!(
                config.select(Some(id), Some("custom-model")).unwrap().model,
                "custom-model"
            );
        }
    }

    #[test]
    fn domestic_profiles_merge_without_clobbering_user_overrides() {
        let mut previous = Config::default();
        previous
            .providers
            .retain(|p| p.id == "openai" || p.id == "gemini");
        assert_eq!(previous.providers.len(), 2);
        previous.include_domestic_api_profiles();
        previous.include_domestic_api_profiles();
        assert_eq!(previous.providers.len(), 7);
        previous.default_provider = "kimi".into();
        previous.validate().unwrap();
        assert_eq!(previous.select(None, None).unwrap().model, "kimi-k3.1-pro");

        let mut custom = previous.select(Some("glm"), None).unwrap();
        custom.model = "my-glm".into();
        custom.base_url = "https://custom.example/v1".into();
        previous.providers.retain(|p| p.id != "glm");
        previous.providers.push(custom);
        previous.include_domestic_api_profiles();
        previous.validate().unwrap();
        assert_eq!(previous.providers.len(), 7);
        assert_eq!(previous.select(Some("glm"), None).unwrap().model, "my-glm");
        assert_eq!(
            previous.select(Some("glm"), None).unwrap().base_url,
            "https://custom.example/v1"
        );
    }

    #[test]
    fn official_model_parameter_constraints_reject_ignored_options() {
        let mut tune = ModelTuning {
            temperature: Some(0.3),
            ..Default::default()
        };
        assert!(tune.validate_for_model("deepseek-flash").is_err());
        assert!(tune.validate_for_model("glm-5.3").is_ok());
        tune.reasoning_effort = Some("none".into());
        assert!(tune.validate_for_model("deepseek-flash").is_ok());
        tune.top_p = Some(0.9);
        assert!(tune.validate_for_model("deepseek-flash").is_err());
        tune.top_p = None;
        tune.temperature = None;
        tune.reasoning_effort = Some("high".into());
        tune.top_p = Some(0.95);
        assert!(tune.validate_for_model("deepseek-flash").is_ok());
        assert!(tune.validate_for_model("deepseek-v4-pro").is_err());
        tune.top_p = None;
        tune.reasoning_effort = Some("none".into());
        assert!(tune.validate_for_model("kimi-k3.1-pro").is_err());
        assert!(tune.validate_for_model("gpt-6.1-sol").is_err());
    }

    #[test]
    fn selecting_custom_model_does_not_mutate_config() {
        let c = Config::default();
        assert_eq!(
            c.select(Some("openai"), Some("example")).unwrap().model,
            "example"
        );
        assert_eq!(c.select(Some("openai"), None).unwrap().model, "gpt-6.1-sol");
    }
}
