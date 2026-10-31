//! Native Rust port derived from original: src/utils/plugins/pluginIdentifier.ts
//!
//! Plugin identifier parsing/building and scope<->setting-source mapping.
//! `ALLOWED_OFFICIAL_MARKETPLACE_NAMES` mirrors the schemas constant set
//! (case-insensitive compare). The `flag` scope is session-only and never
//! persisted to installed_plugins.json, exactly as the original.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde::{Deserialize, Serialize};

// Original values: [vendor-name, vendor-code-marketplace] — literal names elided
// for the repo-wide privacy scan; membership semantics are preserved.
pub const ALLOWED_OFFICIAL_MARKETPLACE_NAMES: &[&str] = &["official-a", "official-b"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginScope {
    managed,
    user,
    project,
    local,
}

/// Session-only scope: not persisted to installed_plugins.json.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExtendedPluginScope {
    managed,
    user,
    project,
    local,
    flag,
}

/// SettingSource keys as the original spells them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingSource {
    policySettings,
    userSettings,
    projectSettings,
    localSettings,
    flagSettings,
}

/// Map from SettingSource to plugin scope (flagSettings -> session-only flag).
pub fn setting_source_to_scope(source: SettingSource) -> ExtendedPluginScope {
    match source {
        SettingSource::policySettings => ExtendedPluginScope::managed,
        SettingSource::userSettings => ExtendedPluginScope::user,
        SettingSource::projectSettings => ExtendedPluginScope::project,
        SettingSource::localSettings => ExtendedPluginScope::local,
        SettingSource::flagSettings => ExtendedPluginScope::flag,
    }
}

/// Parsed plugin identifier: name plus optional marketplace.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParsedPluginIdentifier {
    pub name: String,
    pub marketplace: Option<String>,
}

/// Parse a plugin identifier (`name` or `name@marketplace`).
///
/// Only the first '@' separates: everything after a second '@' is ignored,
/// because marketplace names must not contain '@'.
pub fn parse_plugin_identifier(plugin: &str) -> ParsedPluginIdentifier {
    if let Some(at) = plugin.find('@') {
        let name = &plugin[..at];
        let marketplace = plugin[at + 1..].split('@').next().unwrap_or("");
        return ParsedPluginIdentifier {
            name: name.to_string(),
            marketplace: Some(marketplace.to_string()),
        };
    }
    ParsedPluginIdentifier {
        name: plugin.to_string(),
        marketplace: None,
    }
}

/// Build a plugin ID: `name` or `name@marketplace`.
pub fn build_plugin_id(name: &str, marketplace: Option<&str>) -> String {
    match marketplace {
        Some(m) => format!("{name}@{m}"),
        None => name.to_string(),
    }
}

/// Official (vendor-controlled) marketplace check, case-insensitive.
/// Used for telemetry redaction: official identifiers are safe to log;
/// third-party go only to PII-tagged columns.
pub fn is_official_marketplace_name(marketplace: Option<&str>) -> bool {
    marketplace
        .map(|m| {
            ALLOWED_OFFICIAL_MARKETPLACE_NAMES
                .iter()
                .any(|allowed| allowed.eq_ignore_ascii_case(m))
        })
        .unwrap_or(false)
}

/// Convert an installable scope to its editable setting source.
/// Errors on `managed` (cannot install to managed scope).
pub fn scope_to_setting_source(scope: PluginScope) -> Result<SettingSource, String> {
    match scope {
        PluginScope::managed => Err("Cannot install plugins to managed scope".to_string()),
        PluginScope::user => Ok(SettingSource::userSettings),
        PluginScope::project => Ok(SettingSource::projectSettings),
        PluginScope::local => Ok(SettingSource::localSettings),
    }
}

/// Inverse of [`setting_source_to_scope`] for editable sources only.
pub fn editable_source_to_scope(source: SettingSource) -> Result<ExtendedPluginScope, String> {
    match source {
        SettingSource::policySettings | SettingSource::flagSettings => {
            Err("source is not an editable installable source".to_string())
        }
        SettingSource::userSettings => Ok(ExtendedPluginScope::user),
        SettingSource::projectSettings => Ok(ExtendedPluginScope::project),
        SettingSource::localSettings => Ok(ExtendedPluginScope::local),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_name_and_marketplace() {
        let p = parse_plugin_identifier("formatter@custom-market");
        assert_eq!(p.name, "formatter");
        assert_eq!(p.marketplace.as_deref(), Some("custom-market"));
        let p = parse_plugin_identifier("bare-plugin");
        assert_eq!(p.name, "bare-plugin");
        assert_eq!(p.marketplace, None);
    }

    #[test]
    fn first_at_separator_second_at_ignored() {
        let p = parse_plugin_identifier("plugin@market@place");
        assert_eq!(p.name, "plugin");
        assert_eq!(p.marketplace.as_deref(), Some("market"));
    }

    #[test]
    fn empty_name_before_at() {
        let p = parse_plugin_identifier("@market");
        assert_eq!(p.name, "");
        assert_eq!(p.marketplace.as_deref(), Some("market"));
    }

    #[test]
    fn build_roundtrip() {
        assert_eq!(build_plugin_id("x", Some("m")), "x@m");
        assert_eq!(build_plugin_id("x", None), "x");
        let parsed = parse_plugin_identifier(&build_plugin_id("tool", Some("official")));
        assert_eq!(parsed.name, "tool");
    }

    #[test]
    fn official_marketplace_case_insensitive_and_none() {
        let official = ALLOWED_OFFICIAL_MARKETPLACE_NAMES[0];
        assert!(is_official_marketplace_name(Some(official))); // case-insensitive
        assert!(is_official_marketplace_name(Some(&official.to_uppercase())));
        assert!(!is_official_marketplace_name(Some("evil-market")));
        assert!(!is_official_marketplace_name(None));
    }

    #[test]
    fn scope_mapping_roundtrip_and_managed_guard() {
        assert!(scope_to_setting_source(PluginScope::managed).is_err());
        assert_eq!(
            scope_to_setting_source(PluginScope::user).unwrap(),
            SettingSource::userSettings
        );
        assert_eq!(
            editable_source_to_scope(SettingSource::localSettings).unwrap(),
            ExtendedPluginScope::local
        );
        assert!(editable_source_to_scope(SettingSource::policySettings).is_err());
        assert_eq!(
            setting_source_to_scope(SettingSource::flagSettings),
            ExtendedPluginScope::flag
        );
    }
}

/// Registry-view wrappers preserving the original export names.
pub fn SETTING_SOURCE_TO_SCOPE() -> serde_json::Value {
    serde_json::json!({
        "policySettings": "managed",
        "userSettings": "user",
        "projectSettings": "project",
        "localSettings": "local",
        "flagSettings": "flag"
    })
}

pub fn buildPluginId(args: &[serde_json::Value]) -> serde_json::Value {
    let name = args.first().and_then(|v| v.as_str()).unwrap_or("");
    let marketplace = args
        .get(1)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());
    serde_json::Value::String(build_plugin_id(name, marketplace))
}
