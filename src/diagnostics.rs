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
        Self {
            label,
            ok: true,
            detail: detail.into(),
        }
    }
    fn warning(label: &'static str, detail: impl Into<String>) -> Self {
        Self {
            label,
            ok: false,
            detail: detail.into(),
        }
    }
}

/// Match the source which/Bun.which behavior: a PATH entry must be
/// executable, not merely an existing file. This matters for Doctor and
/// stdio MCP startup diagnostics.
fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn executable_path(program: &str) -> Option<PathBuf> {
    let input = Path::new(program);
    if input.is_absolute() || input.components().count() > 1 {
        return is_executable(input).then(|| input.to_path_buf());
    }
    // Windows where.exe also searches the current directory, even if PATH
    // is absent. POSIX which searches PATH only.
    let mut dirs = Vec::new();
    #[cfg(windows)]
    if let Ok(cwd) = std::env::current_dir() {
        dirs.push(cwd);
    }
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    for dir in &dirs {
        let candidate = dir.join(program);
        if is_executable(&candidate) {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            // where.exe honors PATHEXT, including .COM and user-configured
            // executable suffixes. Prefer its ordering after the exact name.
            if input.extension().is_none() {
                let extensions =
                    std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_owned());
                for suffix in extensions.split(';').filter(|s| !s.is_empty()) {
                    let candidate = dir.join(format!("{program}{suffix}"));
                    if is_executable(&candidate) {
                        return Some(candidate);
                    }
                }
            }
        }
    }
    None
}

fn mcp_check(server: &McpServerConfig) -> Finding {
    match server.transport.as_deref().unwrap_or("stdio") {
        "stdio" => {
            if let Some(file) = executable_path(&server.command) {
                Finding::passed(
                    "MCP",
                    format!(
                        "{}: stdio command available ({})",
                        server.id,
                        file.display()
                    ),
                )
            } else {
                Finding::warning(
                    "MCP",
                    format!(
                        "{}: command '{}' not found in PATH",
                        server.id, server.command
                    ),
                )
            }
        }
        "http" => {
            // Match the connection path: MCP URLs support ${NAME} and
            // ${NAME:-fallback}, and unresolved variables prevent startup.
            let target = server.url.as_deref().unwrap_or("");
            let expanded = crate::compat_helpers::expand_env_vars_in_string(target);
            if !expanded.missing_vars.is_empty() {
                return Finding::warning(
                    "MCP",
                    format!(
                        "{}: missing URL environment variables: {}",
                        server.id,
                        expanded.missing_vars.join(", ")
                    ),
                );
            }
            match reqwest::Url::parse(&expanded.expanded) {
                Ok(url) if matches!(url.scheme(), "http" | "https") => {
                    // Keep the doctor verdict consistent with the real HTTP
                    // transport's preflight checks; never expose URL secrets.
                    if !url.username().is_empty() || url.password().is_some() {
                        return Finding::warning(
                            "MCP",
                            format!("{}: URL credentials are not permitted", server.id),
                        );
                    }
                    for header in server.headers.keys() {
                        if matches!(
                            header.to_ascii_lowercase().as_str(),
                            "accept"
                                | "content-type"
                                | "mcp-session-id"
                                | "mcp-protocol-version"
                                | "host"
                                | "last-event-id"
                        ) {
                            return Finding::warning(
                                "MCP",
                                format!("{}: reserved HTTP header {}", server.id, header),
                            );
                        }
                    }
                    // Do not print the full URL: queries may contain credentials.
                    Finding::passed(
                        "MCP",
                        format!(
                            "{}: HTTP endpoint configured ({})",
                            server.id,
                            url.host_str().unwrap_or("unknown host")
                        ),
                    )
                }
                _ => Finding::warning("MCP", format!("{}: invalid HTTP endpoint", server.id)),
            }
        }
        other => Finding::warning(
            "MCP",
            format!("{}: unsupported transport {other}", server.id),
        ),
    }
}

/// Report specific allow rules that a tool-wide deny/ask rule makes
/// unreachable. Mirrors shadowedRuleDetection.ts: deny wins over ask and
/// tool-wide allow rules are not flagged as shadowed.
fn shadowed_permission_findings(settings: &[crate::permission_rules::RuleSetting]) -> Vec<Finding> {
    use crate::permission_rules::{canonical_tool, compile_rules, Behavior};
    let rules = match compile_rules(settings) {
        Ok(rules) => rules,
        Err(error) => {
            return vec![Finding::warning(
                "Permissions",
                format!("Invalid permission rule configuration: {error}"),
            )];
        }
    };
    let mut findings = Vec::new();
    for rule in &rules {
        if rule.behavior != Behavior::Allow || rule.value.rule_content.is_none() {
            continue;
        }
        let name = canonical_tool(&rule.value.tool_name);
        let blocker = [Behavior::Deny, Behavior::Ask]
            .into_iter()
            .find_map(|behavior| {
                rules.iter().find(|other| {
                    other.behavior == behavior
                        && other.value.rule_content.is_none()
                        && canonical_tool(&other.value.tool_name) == name
                })
            });
        if let Some(blocker) = blocker {
            let action = if blocker.behavior == Behavior::Deny {
                "denied"
            } else {
                "always prompts"
            };
            findings.push(Finding::warning(
                "Permissions",
                format!(
                    "Specific allow rule {} is unreachable: tool-wide {} rule {} {}. Remove the conflicting rule or the ineffective allow.",
                    crate::permission_rules::format_rule(&rule.value),
                    name,
                    crate::permission_rules::format_rule(&blocker.value),
                    action,
                ),
            ));
        }
    }
    findings
}

pub fn inspect(root: &Path, config: Option<&Config>) -> Vec<Finding> {
    let mut findings = vec![Finding::passed("Version", env!("CARGO_PKG_VERSION"))];
    match std::env::current_exe() {
        Ok(exe) => findings.push(Finding::passed("Binary", exe.display().to_string())),
        Err(error) => findings.push(Finding::warning("Binary", format!("{error}"))),
    }
    match root.canonicalize() {
        Ok(path) if path.is_dir() => {
            findings.push(Finding::passed("Workspace", path.display().to_string()))
        }
        Ok(_) => findings.push(Finding::warning("Workspace", "not a directory")),
        Err(error) => findings.push(Finding::warning("Workspace", format!("{error}"))),
    }
    findings.push(match executable_path("git") {
        Some(path) => Finding::passed("Git", format!("available at {}", path.display())),
        None => Finding::warning("Git", "not found in PATH"),
    });
    // Doctor.tsx reports invalid/capped output limits rather than silently
    // treating the effective default as a healthy user configuration.
    // These are the source's provider-neutral Bash and task output limits.
    for (name, default, upper) in [
        ("BASH_MAX_OUTPUT_LENGTH", 30_000, 150_000),
        ("TASK_MAX_OUTPUT_LENGTH", 32_000, 160_000),
    ] {
        let value = std::env::var(name).ok();
        let result = crate::env_validation::validate(name, value.as_deref(), default, upper);
        if result.status != crate::env_validation::ValidationStatus::Valid {
            findings.push(Finding::warning(
                "Environment",
                format!("{name}: {}", result.message.unwrap_or_default()),
            ));
        }
    }
    if let Some(repo) = crate::detect_repository::detect_current_repository_with_host(root) {
        findings.push(Finding::passed(
            "Repository",
            format!("{}/{}/{}", repo.host, repo.owner, repo.name),
        ));
    }
    match Config::path() {
        Ok(path) => {
            if path.is_file() {
                findings.push(Finding::passed(
                    "Settings",
                    format!("local settings file at {}", path.display()),
                ));
            } else {
                findings.push(Finding::warning(
                    "Settings",
                    format!(
                        "not initialized at {}; built-in defaults can still be used",
                        path.display()
                    ),
                ));
            }
        }
        Err(error) => findings.push(Finding::warning("Settings", format!("{error:#}"))),
    }
    if let Some(config) = config {
        findings.push(Finding::passed(
            "Provider",
            format!(
                "{} model profile(s); default {}",
                config.providers.len(),
                config.default_provider
            ),
        ));
        for server in &config.mcp_servers {
            findings.push(mcp_check(server));
        }
        // Include effective user/project/local/managed settings, not only the
        // global provider configuration, when explaining ineffective rules.
        match crate::permission_settings::SettingsPaths::for_workspace(root)
            .and_then(|paths| paths.load(&config.permission_rules))
        {
            Ok(rules) => findings.extend(shadowed_permission_findings(&rules)),
            Err(error) => findings.push(Finding::warning(
                "Permissions",
                format!("Unable to inspect active permission rules: {error:#}"),
            )),
        }
        if let Some(shell) = &config.default_shell {
            findings.push(Finding::passed(
                "Shell",
                format!("selected runtime: {shell}"),
            ));
        }
    } else {
        findings.push(Finding::warning(
            "Provider",
            "settings could not be loaded; check configuration syntax and values",
        ));
    }
    for relative in ["AGENTS.md", ".atlas/instructions.md"] {
        let file = root.join(relative);
        match fs::metadata(&file) {
            Ok(meta) if !meta.is_file() => {
                findings.push(Finding::warning(
                    "Context",
                    format!("{relative} is not a regular file"),
                ));
            }
            Ok(meta) if meta.len() > 50_000 => {
                findings.push(Finding::warning(
                    "Context",
                    format!("{relative} is {} bytes (max 50000)", meta.len()),
                ));
            }
            Ok(meta) => findings.push(Finding::passed(
                "Context",
                format!("{relative}: {} bytes", meta.len()),
            )),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                findings.push(Finding::warning("Context", format!("{relative}: {error}")))
            }
        }
    }
    // doctorContextWarnings.ts uses the decoded memory text length (UTF-16
    // units in JavaScript), not filesystem bytes, and warns above 40k.
    // Keep the existing size diagnostics while reporting this independent
    // prompt-context risk for provider-neutral instruction files.
    for relative in ["AGENTS.md", ".atlas/instructions.md"] {
        let file = root.join(relative);
        if let Ok(content) = fs::read_to_string(&file) {
            let characters = content.encode_utf16().count();
            if characters > 40_000 {
                findings.push(Finding::warning(
                    "Memory",
                    format!("{relative}: {characters} characters (>40000)"),
                ));
            }
        }
    }
    findings
}

pub fn render(findings: &[Finding]) -> String {
    let mut lines = vec![format!(
        "Environment diagnostics ({} checks)",
        findings.len()
    )];
    for finding in findings {
        lines.push(format!(
            "[{}] {}: {}",
            if finding.ok { "OK" } else { "WARN" },
            finding.label,
            finding.detail
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
        }))
        .unwrap()];
        let report = render(&inspect(root.path(), Some(&config)));
        assert!(
            report.contains("Context: AGENTS.md is 50001 bytes"),
            "{report}"
        );
        assert!(report.contains("MCP: missing-tool: command"), "{report}");
        assert!(report.contains("Provider: 3 model profile"), "{report}");
        assert!(!report.contains("nonexistent-secret"));
    }

    #[test]
    fn http_mcp_reports_host_without_secret_query() {
        let server: McpServerConfig = serde_json::from_value(json!({
            "id":"remote", "type":"http",
            "url":"https://example.test/mcp?token=SECRET_QUERY_MUST_NOT_APPEAR"
        }))
        .unwrap();
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
