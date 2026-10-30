//! Source-parity error diagnostics from src/commands/plugin/PluginErrors.tsx.
//! This provider-neutral implementation preserves actionable descriptions and
//! remedies instead of hiding the actual error behind generic messages.
use anyhow::{anyhow, bail, Result};
use serde_json::Value;

fn kind(error: &Value) -> Result<&str> {
    field(error, "type")
}
fn field<'a>(error: &'a Value, name: &str) -> Result<&'a str> {
    error
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("plugin diagnostic missing string field {name}"))
}
fn list(error: &Value, name: &str) -> Result<String> {
    error
        .get(name)
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("plugin diagnostic missing list field {name}"))?
        .iter()
        .map(|v| {
            v.as_str()
                .ok_or_else(|| anyhow!("plugin diagnostic has invalid {name} entry"))
        })
        .collect::<Result<Vec<_>>>()
        .map(|v| v.join(", "))
}
fn winning_plugin(duplicate: &str) -> &str {
    duplicate.split(':').nth(1).unwrap_or("?")
}

pub fn format_error_message(error: &Value) -> Result<String> {
    let message = match kind(error)? {
        "path-not-found" => format!(
            "{} path not found: {}",
            field(error, "component")?,
            field(error, "path")?
        ),
        "git-auth-failed" => format!(
            "Git {} authentication failed for {}",
            field(error, "authType")?.to_uppercase(),
            field(error, "gitUrl")?
        ),
        "git-timeout" => format!(
            "Git {} timed out for {}",
            field(error, "operation")?,
            field(error, "gitUrl")?
        ),
        "network-error" => format!(
            "Network error accessing {}{}",
            field(error, "url")?,
            error
                .get("details")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(|s| format!(": {s}"))
                .unwrap_or_default()
        ),
        "manifest-parse-error" => format!(
            "Failed to parse manifest at {}: {}",
            field(error, "manifestPath")?,
            field(error, "parseError")?
        ),
        "manifest-validation-error" => format!(
            "Invalid manifest at {}: {}",
            field(error, "manifestPath")?,
            list(error, "validationErrors")?
        ),
        "plugin-not-found" => format!(
            "Plugin \"{}\" not found in marketplace \"{}\"",
            field(error, "pluginId")?,
            field(error, "marketplace")?
        ),
        "marketplace-not-found" => {
            format!("Marketplace \"{}\" not found", field(error, "marketplace")?)
        }
        "marketplace-load-failed" => format!(
            "Failed to load marketplace \"{}\": {}",
            field(error, "marketplace")?,
            field(error, "reason")?
        ),
        "mcp-config-invalid" => format!(
            "Invalid MCP server config for \"{}\": {}",
            field(error, "serverName")?,
            field(error, "validationError")?
        ),
        "mcp-server-suppressed-duplicate" => {
            let dup = field(error, "duplicateOf")?;
            let desc = if dup.starts_with("plugin:") {
                format!("server provided by plugin \"{}\"", winning_plugin(dup))
            } else {
                format!("already-configured \"{dup}\"")
            };
            format!(
                "MCP server \"{}\" skipped — same command/URL as {desc}",
                field(error, "serverName")?
            )
        }
        "hook-load-failed" => format!(
            "Failed to load hooks from {}: {}",
            field(error, "hookPath")?,
            field(error, "reason")?
        ),
        "component-load-failed" => format!(
            "Failed to load {} from {}: {}",
            field(error, "component")?,
            field(error, "path")?,
            field(error, "reason")?
        ),
        "mcpb-download-failed" => format!(
            "Failed to download MCPB from {}: {}",
            field(error, "url")?,
            field(error, "reason")?
        ),
        "mcpb-extract-failed" => format!(
            "Failed to extract MCPB {}: {}",
            field(error, "mcpbPath")?,
            field(error, "reason")?
        ),
        "mcpb-invalid-manifest" => format!(
            "MCPB manifest invalid at {}: {}",
            field(error, "mcpbPath")?,
            field(error, "validationError")?
        ),
        "marketplace-blocked-by-policy" => {
            if error.get("blockedByBlocklist").and_then(Value::as_bool) == Some(true) {
                format!(
                    "Marketplace \"{}\" is blocked by enterprise policy",
                    field(error, "marketplace")?
                )
            } else {
                format!(
                    "Marketplace \"{}\" is not in the allowed marketplace list",
                    field(error, "marketplace")?
                )
            }
        }
        "dependency-unsatisfied" => format!(
            "Dependency \"{}\" is {}",
            field(error, "dependency")?,
            if field(error, "reason")? == "not-enabled" {
                "disabled"
            } else {
                "not installed"
            }
        ),
        "lsp-config-invalid" => format!(
            "Invalid LSP server config for \"{}\": {}",
            field(error, "serverName")?,
            field(error, "validationError")?
        ),
        "lsp-server-start-failed" => format!(
            "LSP server \"{}\" failed to start: {}",
            field(error, "serverName")?,
            field(error, "reason")?
        ),
        "lsp-server-crashed" => {
            if let Some(signal) = error
                .get("signal")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
            {
                format!(
                    "LSP server \"{}\" crashed with signal {signal}",
                    field(error, "serverName")?
                )
            } else {
                let exit = error
                    .get("exitCode")
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "unknown".into());
                format!(
                    "LSP server \"{}\" crashed with exit code {exit}",
                    field(error, "serverName")?
                )
            }
        }
        "lsp-request-timeout" => format!(
            "LSP server \"{}\" timed out on {} after {}ms",
            field(error, "serverName")?,
            field(error, "method")?,
            error
                .get("timeoutMs")
                .and_then(Value::as_u64)
                .ok_or_else(|| anyhow!("missing timeoutMs"))?
        ),
        "lsp-request-failed" => format!(
            "LSP server \"{}\" {} failed: {}",
            field(error, "serverName")?,
            field(error, "method")?,
            field(error, "error")?
        ),
        "plugin-cache-miss" => format!(
            "Plugin \"{}\" not cached at {}",
            field(error, "plugin")?,
            field(error, "installPath")?
        ),
        "generic-error" => field(error, "error")?.into(),
        other => bail!("unknown plugin error variant: {other}"),
    };
    Ok(message)
}

pub fn error_guidance(error: &Value) -> Result<Option<String>> {
    let result: Option<String> = match kind(error)? {
        "path-not-found" => {
            Some("Check that the path in your manifest or marketplace config is correct".into())
        }
        "git-auth-failed" => Some(
            if field(error, "authType")? == "ssh" {
                "Configure SSH keys or use HTTPS URL instead"
            } else {
                "Configure credentials or use SSH URL instead"
            }
            .into(),
        ),
        "git-timeout" | "network-error" => {
            Some("Check your internet connection and try again".into())
        }
        "manifest-parse-error" => Some("Check manifest file syntax in the plugin directory".into()),
        "manifest-validation-error" => {
            Some("Check manifest file follows the required schema".into())
        }
        "plugin-not-found" => Some(format!(
            "Plugin may not exist in marketplace \"{}\"",
            field(error, "marketplace")?
        )),
        "marketplace-not-found" => Some(
            if error
                .get("availableMarketplaces")
                .and_then(Value::as_array)
                .is_some_and(|a| !a.is_empty())
            {
                format!(
                    "Available marketplaces: {}",
                    list(error, "availableMarketplaces")?
                )
            } else {
                "Add the marketplace first using /plugin marketplace add".into()
            },
        ),
        "mcp-config-invalid" => {
            Some("Check MCP server configuration in .mcp.json or manifest".into())
        }
        "mcp-server-suppressed-duplicate" => {
            let dup = field(error, "duplicateOf")?;
            Some(if dup.starts_with("plugin:") {
                format!(
                    "Disable plugin \"{}\" if you want this plugin's version instead",
                    winning_plugin(dup)
                )
            } else {
                format!("Remove \"{dup}\" from your MCP config if you want the plugin's version instead")
            })
        }
        "hook-load-failed" => Some("Check hooks.json file syntax and structure".into()),
        "component-load-failed" => Some(format!(
            "Check {} directory structure and file permissions",
            field(error, "component")?
        )),
        "mcpb-download-failed" => {
            Some("Check your internet connection and URL accessibility".into())
        }
        "mcpb-extract-failed" => Some("Verify the MCPB file is valid and not corrupted".into()),
        "mcpb-invalid-manifest" => {
            Some("Contact the plugin author about the invalid manifest".into())
        }
        "marketplace-blocked-by-policy" => Some(
            if error.get("blockedByBlocklist").and_then(Value::as_bool) == Some(true) {
                "This marketplace source is explicitly blocked by your administrator".into()
            } else if error
                .get("allowedSources")
                .and_then(Value::as_array)
                .is_some_and(|a| !a.is_empty())
            {
                format!("Allowed sources: {}", list(error, "allowedSources")?)
            } else {
                "Contact your administrator to configure allowed marketplace sources".into()
            },
        ),
        "dependency-unsatisfied" => Some(if field(error, "reason")? == "not-enabled" {
            format!(
                "Enable \"{}\" or uninstall \"{}\"",
                field(error, "dependency")?,
                field(error, "plugin")?
            )
        } else {
            format!(
                "Install \"{}\" or uninstall \"{}\"",
                field(error, "dependency")?,
                field(error, "plugin")?
            )
        }),
        "lsp-config-invalid" => {
            Some("Check LSP server configuration in the plugin manifest".into())
        }
        "lsp-server-start-failed"
        | "lsp-server-crashed"
        | "lsp-request-timeout"
        | "lsp-request-failed" => Some("Check LSP server logs with --debug for details".into()),
        "plugin-cache-miss" => Some("Run /plugins to refresh the plugin cache".into()),
        "marketplace-load-failed" | "generic-error" => None,
        other => bail!("unknown plugin error variant: {other}"),
    };
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn formats_each_category_of_plugin_failure() {
        let cases = [
            (json!({"type":"path-not-found","component":"skills","path":"/x"}),"skills path not found: /x"),
            (json!({"type":"git-auth-failed","authType":"ssh","gitUrl":"repo"}),"Git SSH authentication failed for repo"),
            (json!({"type":"mcp-server-suppressed-duplicate","serverName":"s","duplicateOf":"plugin:other:server"}),"MCP server \"s\" skipped — same command/URL as server provided by plugin \"other\""),
            (json!({"type":"lsp-request-timeout","serverName":"language","method":"completion","timeoutMs":1500}),"LSP server \"language\" timed out on completion after 1500ms"),
            (json!({"type":"dependency-unsatisfied","dependency":"dep","reason":"not-enabled"}),"Dependency \"dep\" is disabled"),
            (json!({"type":"generic-error","error":"details"}),"details"),
        ];
        for (input, expected) in cases {
            assert_eq!(format_error_message(&input).unwrap(), expected);
        }
    }
    #[test]
    fn guidance_respects_policy_and_declines_unknown_variants() {
        assert_eq!(
            error_guidance(
                &json!({"type":"marketplace-not-found","availableMarketplaces":["one","two"]})
            )
            .unwrap()
            .as_deref(),
            Some("Available marketplaces: one, two")
        );
        assert_eq!(
            error_guidance(&json!({"type":"generic-error"})).unwrap(),
            None
        );
        assert!(format_error_message(
            &json!({"type":"manifest-validation-error","manifestPath":"x"})
        )
        .is_err());
        assert!(error_guidance(&json!({"type":"unknown"})).is_err());
    }
}
