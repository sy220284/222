//! Native Rust port derived from original: src/utils/settings/settingsCache.ts
//!
//! Session-scoped cache for parsed project settings, keyed by the three
//! merged source maps. Global mutable state mirrors the original module
//! singletons; the settings document itself is `serde_json::Value`
//! (the zod-validated `SettingsJson` shape lives in the settings types
//! port and is not re-validated here — callers pass parsed documents).
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::OnceLock;

use serde_json::Value;

/// One settings document plus its parse errors, mirroring
/// `SettingsWithErrors`.
#[derive(Debug, Clone, Default)]
pub struct SettingsWithErrors {
    pub settings: Value,
    pub errors: Vec<String>,
    pub file_path: String,
}

/// Source key mirrors the original `SettingSource` union
/// (userSettings/projectSettings/localSettings/flagSettings/policySettings).
pub type SettingSourceKey = String;

fn cache_slot() -> &'static Mutex<SettingsCacheState> {
    static SLOT: OnceLock<Mutex<SettingsCacheState>> = OnceLock::new();
    SLOT.get_or_init(|| {
        Mutex::new(SettingsCacheState {
            session: None,
            project: HashMap::new(),
        })
    })
}

#[derive(Debug, Default)]
struct SettingsCacheState {
    session: Option<SettingsWithErrors>,
    /// Keyed by "<source>|<path>" — mirrors the original two keyed maps
    /// (project settings and their errors per source).
    project: HashMap<String, SettingsWithErrors>,
}

/// Returns the session settings cache entry, if present.
pub fn get_session_settings_cache() -> Option<SettingsWithErrors> {
    cache_slot().lock().unwrap().session.clone()
}

/// Replaces the session settings cache entry.
pub fn set_session_settings_cache(value: Option<SettingsWithErrors>) {
    cache_slot().lock().unwrap().session = value;
}

/// Returns a cached project-settings entry for the given source and path.
pub fn get_project_settings_cache(source: &str, path: &str) -> Option<SettingsWithErrors> {
    let state = cache_slot().lock().unwrap();
    state.project.get(&format!("{source}|{path}")).cloned()
}

/// Stores a project-settings cache entry for the given source and path.
pub fn set_project_settings_cache(source: &str, path: &str, value: SettingsWithErrors) {
    let mut state = cache_slot().lock().unwrap();
    state.project.insert(format!("{source}|{path}"), value);
}

/// Evicts every cached project-settings entry (used when settings files
/// change on disk; the original clears the whole map).
pub fn clear_project_settings_cache() {
    cache_slot().lock().unwrap().project.clear();
}

/// Clears all settings caches (session + project).
pub fn reset_all_settings_caches() {
    let mut state = cache_slot().lock().unwrap();
    state.session = None;
    state.project.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_cache_roundtrip_and_reset() {
        reset_all_settings_caches();
        assert!(get_session_settings_cache().is_none());
        let doc = SettingsWithErrors {
            settings: serde_json::json!({"model": "claude"}),
            errors: vec![],
            file_path: "/tmp/settings.json".into(),
        };
        set_session_settings_cache(Some(doc));
        let got = get_session_settings_cache().unwrap();
        assert_eq!(got.settings["model"], "claude");
        reset_all_settings_caches();
        assert!(get_session_settings_cache().is_none());
    }

    #[test]
    fn project_cache_is_keyed_by_source_and_path() {
        reset_all_settings_caches();
        let doc = SettingsWithErrors {
            settings: serde_json::json!({"permissions": {}}),
            errors: vec!["bad field".into()],
            file_path: "/repo/.claude/settings.json".into(),
        };
        set_project_settings_cache("projectSettings", "/repo/.claude/settings.json", doc);
        let got =
            get_project_settings_cache("projectSettings", "/repo/.claude/settings.json").unwrap();
        assert_eq!(got.errors, vec!["bad field"]);
        assert!(
            get_project_settings_cache("userSettings", "/repo/.claude/settings.json").is_none()
        );
        clear_project_settings_cache();
        assert!(
            get_project_settings_cache("projectSettings", "/repo/.claude/settings.json").is_none()
        );
    }
}
