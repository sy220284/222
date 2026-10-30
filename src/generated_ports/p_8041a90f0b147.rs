//! Native Rust port derived from original: src/utils/caCertsConfig.ts
//!
//! Settings-backed `NODE_EXTRA_CA_CERTS` population applied early in init,
//! before any TLS connection. The original reads `getGlobalConfig()` and
//! `getSettingsForSource('userSettings')`; those loaders live behind the
//! config/settings ports, so this port takes the two already-parsed `env`
//! objects and applies the same precedence: settings override global
//! config; a pre-existing process env value always wins; only
//! user-controlled sources are consulted (never project settings).
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde_json::Value;

use crate::generated_ports::p_5b257a1951fde::{log_for_debugging, DebugLogLevel};

pub const NODE_EXTRA_CA_CERTS: &str = "NODE_EXTRA_CA_CERTS";

fn env_map_get(env: Option<&Value>, key: &str) -> Option<String> {
    env.and_then(|v| v.get(key))
        .and_then(|v| v.as_str())
        .map(str::to_owned)
        .filter(|s| !s.is_empty())
}

/// Reads NODE_EXTRA_CA_CERTS from settings/config as a fallback.
/// `settings_env` mirrors `getSettingsForSource('userSettings')?.env`,
/// `global_env` mirrors `getGlobalConfig()?.env`.
pub fn get_extra_certs_path_from_config(
    settings_env: Option<&Value>,
    global_env: Option<&Value>,
) -> Option<String> {
    let settings_keys = settings_env
        .and_then(|v| v.as_object())
        .map(|o| o.keys().cloned().collect::<Vec<_>>().join(","))
        .unwrap_or_else(|| "none".to_string());
    let global_keys = global_env
        .and_then(|v| v.as_object())
        .map(|o| o.keys().cloned().collect::<Vec<_>>().join(","))
        .unwrap_or_else(|| "none".to_string());
    log_for_debugging(
        &format!(
            "CA certs: Config fallback - globalEnv keys: {}, settingsEnv keys: {}",
            global_keys, settings_keys
        ),
        DebugLogLevel::debug,
    );
    // Settings override global config (same precedence as
    // applyConfigEnvironmentVariables).
    let path = env_map_get(settings_env, NODE_EXTRA_CA_CERTS)
        .or_else(|| env_map_get(global_env, NODE_EXTRA_CA_CERTS));
    if let Some(p) = &path {
        log_for_debugging(
            &format!(
                "CA certs: Found NODE_EXTRA_CA_CERTS in config/settings: {}",
                p
            ),
            DebugLogLevel::debug,
        );
    }
    path
}

/// Applies NODE_EXTRA_CA_CERTS from settings to the process env early in
/// init, BEFORE any TLS connections are made. Returns whether the env var
/// was set by this call. Safe before the trust dialog: only user-controlled
/// files feed `settings_env`/`global_env` here, never project settings.
pub fn apply_extra_ca_certs_from_config(
    settings_env: Option<&Value>,
    global_env: Option<&Value>,
) -> bool {
    if std::env::var_os(NODE_EXTRA_CA_CERTS).is_some_and(|v| !v.is_empty()) {
        return false; // Already set in environment, nothing to do
    }
    match get_extra_certs_path_from_config(settings_env, global_env) {
        Some(config_path) => {
            std::env::set_var(NODE_EXTRA_CA_CERTS, &config_path);
            log_for_debugging(
                &format!(
                    "CA certs: Applied NODE_EXTRA_CA_CERTS from config to process.env: {}",
                    config_path
                ),
                DebugLogLevel::debug,
            );
            true
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn settings_override_global_config() {
        let settings = json!({ NODE_EXTRA_CA_CERTS: "/from/settings.pem" });
        let global = json!({ NODE_EXTRA_CA_CERTS: "/from/global.pem" });
        let path = get_extra_certs_path_from_config(Some(&settings), Some(&global));
        assert_eq!(path.as_deref(), Some("/from/settings.pem"));
    }

    #[test]
    fn global_used_when_settings_missing() {
        let global = json!({ NODE_EXTRA_CA_CERTS: "/from/global.pem" });
        let path = get_extra_certs_path_from_config(None, Some(&global));
        assert_eq!(path.as_deref(), Some("/from/global.pem"));
    }

    #[test]
    fn empty_value_is_ignored() {
        let settings = json!({ NODE_EXTRA_CA_CERTS: "" });
        assert_eq!(
            get_extra_certs_path_from_config(Some(&settings), None),
            None
        );
    }

    #[test]
    fn apply_respects_preexisting_env() {
        std::env::set_var(NODE_EXTRA_CA_CERTS, "/already/set.pem");
        let settings = json!({ NODE_EXTRA_CA_CERTS: "/from/settings.pem" });
        assert!(!apply_extra_ca_certs_from_config(Some(&settings), None));
        assert_eq!(
            std::env::var(NODE_EXTRA_CA_CERTS).as_deref(),
            Ok("/already/set.pem")
        );
        std::env::remove_var(NODE_EXTRA_CA_CERTS);
    }

    #[test]
    fn apply_sets_env_from_settings() {
        std::env::remove_var(NODE_EXTRA_CA_CERTS);
        let settings = json!({ NODE_EXTRA_CA_CERTS: "/applied.pem" });
        assert!(apply_extra_ca_certs_from_config(Some(&settings), None));
        assert_eq!(
            std::env::var(NODE_EXTRA_CA_CERTS).as_deref(),
            Ok("/applied.pem")
        );
        std::env::remove_var(NODE_EXTRA_CA_CERTS);
    }
}
