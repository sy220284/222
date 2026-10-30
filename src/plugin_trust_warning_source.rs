//! Manual port of src/commands/plugin/PluginTrustWarning.tsx.
//! The original component reads a policy-scoped notice and displays a static
//! warning in italic/dim text. Provider-specific brand language is removed.
use serde_json::Value;

pub const PLUGIN_TRUST_WARNING: &str = "Make sure you trust a plugin before installing, updating, or using it. Plugin providers cannot control what MCP servers, files, or other software are included in third-party plugins and cannot verify that they will work as intended or that they won't change. See each plugin's homepage for more information.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginTrustWarning {
    pub icon: &'static str,
    pub message: String,
    pub margin_bottom: usize,
    pub dimmed: bool,
    pub italic: bool,
}

pub fn policy_plugin_trust_message(settings: &Value) -> Option<&str> {
    settings
        .get("policySettings")
        .and_then(|policy| policy.get("pluginTrustMessage"))
        .and_then(Value::as_str)
}

pub fn plugin_trust_warning(settings: &Value) -> PluginTrustWarning {
    let custom = policy_plugin_trust_message(settings).filter(|text| !text.is_empty());
    let mut message = PLUGIN_TRUST_WARNING.to_owned();
    if let Some(extra) = custom {
        message.push(' ');
        message.push_str(extra);
    }
    PluginTrustWarning {
        icon: "⚠ ",
        message,
        margin_bottom: 1,
        dimmed: true,
        italic: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn uses_only_policy_message_and_preserves_inline_spacing() {
        let from_user = json!({"userSettings":{"pluginTrustMessage":"ignored"}});
        assert_eq!(
            plugin_trust_warning(&from_user).message,
            PLUGIN_TRUST_WARNING
        );
        let policy = json!({"policySettings":{"pluginTrustMessage":"Admin-approved plugins only"}});
        let displayed = plugin_trust_warning(&policy);
        assert_eq!(
            displayed.message,
            format!("{PLUGIN_TRUST_WARNING} Admin-approved plugins only")
        );
        assert_eq!(displayed.icon, "⚠ ");
        assert_eq!(displayed.margin_bottom, 1);
        assert!(displayed.italic && displayed.dimmed);
        let empty = json!({"policySettings":{"pluginTrustMessage":""}});
        assert_eq!(plugin_trust_warning(&empty).message, PLUGIN_TRUST_WARNING);
    }
}
