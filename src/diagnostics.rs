//! Local environment checks corresponding to the original Doctor screen.
//! All checks are offline and do not run installer or updater side effects.
use crate::config::{Config, McpServerConfig};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub label: &'static str,
    pub ok: bool,
    pub detail: String,
}

impl Finding {
    fn passed(label: &'static str, detail: impl Into<String>) -> Self {
        Self { label, ok: true, detail: detail.into() }
    }
    fn warning(label: &'static str, detail: impl Into<String>) -> Self {
        Self { label, ok: false, detail: detail.into() }
    }
}

fn executable_path(program: &str) -> Option<PathBuf> {
    let input = Path::new(program);
    if input.is_absolute() || input.components().count() > 1 {
        return input.is_file().then(|| input.to_path_buf());
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(program))
        .find(|path| path.is_file())
        .or_else(|| {
            #[cfg(windows)]
            {
                std::env::split_paths(&path)
                    .flat_map(|dir| ["exe", "cmd", "bat"].into_iter()
                        .map(move |suffix| dir.join(format!("{program}.{suffix}"))))
                    .find(|candidate| candidate.is_file())
            }
            #[cfg(not(windows))]
            {
                None
            }
        })
}

fn mcp_check(server: &McpServerConfig) -> Finding {
    match server.transport.as_deref().unwrap_or("stdio") {
        "stdio" => {
            if let Some(file) = executable_path(&server.command) {
                Finding::passed("MCP", format!("{}: stdio command available ({})",
                    server.id, file.display()))
            } else {
                Finding::warning("MCP", format!(
                    "{}: command '{}' not found in PATH", server.id, server.command
                ))
            }
        }
        "http" => {
            let target = server.url.as_deref().unwrap_or("");
            match reqwest::Url::parse(target) {
                Ok(url) if matches!(url.scheme(), "http" | "https") => {
                    // Do not print the full URL: queries may contain credentials.
                    Finding::passed("MCP", format!("{}: HTTP endpoint configured ({})",
                        server.id, url.host_str().unwrap_or("unknown host")))
                }
                _ => Finding::warning("MCP", format!(
                    "{}: invalid HTTP endpoint", server.id
                )),
            }
        }
        other => Finding::warning("MCP", format!(
            "{}: unsupported transport {other}", server.id
        )),
    }
}

pub fn inspect(root: &Path, config: Option<&Config>) -> Vec<Finding> {
    let mut findings = vec![Finding::passed("Version", env!("CARGO_PKG_VERSION"))];
    match std::env::current_exe() {
        Ok(exe) => findings.push(Finding::passed("Binary", exe.display().to_string())),
        Err(error) => findings.push(Finding::warning("Binary", format!("{error}"))),
    }
    match root.canonicalize() {
        Ok(path) if path.is_dir() => findings.push(Finding::passed(
            "Workspace", path.display().to_string())),
        Ok(_) => findings.push(Finding::warning("Workspace", "not a directory")),
        Err(error) => findings.push(Finding::warning("Workspace", format!("{error}"))),
    }
    findings.push(match executable_path("git") {
        Some(path) => Finding::passed("Git", format!("available at {}", path.display())),
        None => Finding::warning("Git", "not found in PATH"),
    });
    match Config::path() {
        Ok(path) => {
            if path.is_file() {
                findings.push(Finding::passed("Settings", format!(
                    "local settings file at {}", path.display()
                )));
            } else {
                findings.push(Finding::warning("Settings", format!(
                    "not initialized at {}; built-in defaults can still be used",
                    path.display()
                )));
            }
        }
        Err(error) => findings.push(Finding::warning("Settings", format!("{error:#}"))),
    }
    if let Some(config) = config {
        findings.push(Finding::passed("Provider", format!(
            "{} model profile(s); default {}", config.providers.len(), config.default_provider
        )));
        for server in &config.mcp_servers {
            findings.push(mcp_check(server));
        }
        if let Some(shell) = &config.default_shell {
            findings.push(Finding::passed("Shell", format!(
                "selected runtime: {shell}"
            )));
        }
    } else {
        findings.push(Finding::warning(
            "Provider", "settings could not be loaded; check configuration syntax and values"
        ));
    }
    for relative in ["AGENTS.md", ".atlas/instructions.md"] {
        let file = root.join(relative);
        match fs::metadata(&file) {
            Ok(meta) if !meta.is_file() => {
                findings.push(Finding::warning("Context", format!(
                    "{relative} is not a regular file"
                )));
            }
            Ok(meta) if meta.len() > 50_000 => {
                findings.push(Finding::warning("Context", format!(
                    "{relative} is {} bytes (max 50000)", meta.len()
                )));
            }
            Ok(meta) => findings.push(Finding::passed("Context", format!(
                "{relative}: {} bytes", meta.len()
            ))),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => findings.push(Finding::warning("Context", format!(
                "{relative}: {error}"
            ))),
        }
    }
    findings
}

pub fn render(findings: &[Finding]) -> String {
    let mut lines = vec![format!("Environment diagnostics ({} checks)", findings.len())];
    for finding in findings {
        lines.push(format!(
            "[{}] {}: {}", if finding.ok { "OK" } else { "WARN" },
            finding.label, finding.detail
        ));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reports_configuration_context_and_missing_mcp_without_network() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("AGENTS.md"), "x".repeat(50_001)).unwrap();
        let mut config = Config::default();
        config.mcp_servers = vec![serde_json::from_value(json!({
            "id":"missing-tool","command":"nonexistent-mcp-server-test-84389"
        })).unwrap()];
        let report = render(&inspect(root.path(), Some(&config)));
        assert!(report.contains("Context: AGENTS.md is 50001 bytes"), "{report}");
        assert!(report.contains("MCP: missing-tool: command"), "{report}");
        assert!(report.contains("Provider: 3 model profile"), "{report}");
        assert!(!report.contains("nonexistent-secret"));
    }

    #[test]
    fn http_mcp_reports_host_without_secret_query() {
        let server: McpServerConfig = serde_json::from_value(json!({
            "id":"remote", "type":"http",
            "url":"https://example.test/mcp?token=SECRET_QUERY_MUST_NOT_APPEAR"
        })).unwrap();
        let result = mcp_check(&server);
        assert!(result.ok);
        assert!(result.detail.contains("example.test"));
        assert!(!result.detail.contains("SECRET_QUERY_MUST_NOT_APPEAR"));
    }

    #[test]
    fn missing_root_is_reported_not_created() {
        let root = tempfile::tempdir().unwrap();
        let missing = root.path().join("missing-workspace");
        let findings = inspect(&missing, None);
        assert!(findings.iter().any(|f| f.label == "Workspace" && !f.ok));
        assert!(!missing.exists());
    }
}
