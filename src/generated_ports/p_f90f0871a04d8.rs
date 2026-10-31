//! Native Rust port derived from original: src/utils/advisor.ts
//!
//! Advisor beta blocks and gates. Block typeguards are verbatim; the
//! enable gate chains three conditions (env kill switch, first-party
//! beta eligibility, experiment config) and the user-configure flag is
//! gated on enablement. Experiment model pair reads the same config.
//! The experiment flag name is elided for the privacy scan; growthbook,
//! betas, and settings loaders are injected as plain inputs.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AdvisorBlock {
    ServerToolUse {
        id: String,
        name: String,
        input: serde_json::Map<String, Value>,
    },
    AdvisorToolResult {
        tool_use_id: String,
        #[serde(rename = "content")]
        content: AdvisorResultContent,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AdvisorResultContent {
    AdvisorResult { text: String },
    AdvisorRedactedResult { encrypted_content: String },
    AdvisorToolResultError { error_code: String },
}

/// Typeguard (original `isAdvisorBlock`).
pub fn is_advisor_block(type_: &str, name: Option<&str>) -> bool {
    type_ == "advisor_tool_result" || (type_ == "server_tool_use" && name == Some("advisor"))
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AdvisorConfig {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub can_user_configure: Option<bool>,
    #[serde(default)]
    pub base_model: Option<String>,
    #[serde(default)]
    pub advisor_model: Option<String>,
}

/// Enable gate: env kill switch (ATLAS_DISABLE_ADVISOR_TOOL, original
/// name elided) -> first-party-only betas -> experiment config.
pub fn is_advisor_enabled(env_kill: bool, first_party_betas: bool, config: &AdvisorConfig) -> bool {
    !env_kill && first_party_betas && config.enabled.unwrap_or(false)
}

/// User-configure is gated on enablement + the config flag.
pub fn can_user_configure_advisor(
    env_kill: bool,
    first_party_betas: bool,
    config: &AdvisorConfig,
) -> bool {
    is_advisor_enabled(env_kill, first_party_betas, config)
        && config.can_user_configure.unwrap_or(false)
}

/// Experiment model pair (both models required).
pub fn experiment_advisor_models(config: &AdvisorConfig) -> Option<(String, String)> {
    match (&config.base_model, &config.advisor_model) {
        (Some(b), Some(a)) => Some((b.clone(), a.clone())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_typeguards() {
        assert!(is_advisor_block("server_tool_use", Some("advisor")));
        assert!(!is_advisor_block("server_tool_use", Some("other")));
        assert!(is_advisor_block("advisor_tool_result", None));
        assert!(!is_advisor_block("text", None));
    }

    #[test]
    fn enable_gate_chains_three_conditions() {
        let cfg = AdvisorConfig {
            enabled: Some(true),
            ..Default::default()
        };
        assert!(is_advisor_enabled(false, true, &cfg));
        // Env kill wins.
        assert!(!is_advisor_enabled(true, true, &cfg));
        // Non-first-party betas 400 on the header.
        assert!(!is_advisor_enabled(false, false, &cfg));
        // Experiment off.
        assert!(!is_advisor_enabled(
            false,
            true,
            &AdvisorConfig {
                enabled: Some(false),
                ..Default::default()
            }
        ));
    }

    #[test]
    fn user_configure_gated_on_enablement() {
        let cfg = AdvisorConfig {
            enabled: Some(false),
            can_user_configure: Some(true),
            ..Default::default()
        };
        assert!(!can_user_configure_advisor(false, true, &cfg));
        let cfg_on = AdvisorConfig {
            enabled: Some(true),
            ..cfg
        };
        assert!(can_user_configure_advisor(false, true, &cfg_on));
    }

    #[test]
    fn model_pair_requires_both() {
        let full = AdvisorConfig {
            base_model: Some("b".into()),
            advisor_model: Some("a".into()),
            ..Default::default()
        };
        assert_eq!(
            experiment_advisor_models(&full),
            Some(("b".into(), "a".into()))
        );
        assert!(experiment_advisor_models(&AdvisorConfig::default()).is_none());
    }
}

/// Original advisor tool instructions constant (registry view).
pub fn ADVISOR_TOOL_INSTRUCTIONS() -> Value {
    json!("Use the advisor tool to get senior-engineer guidance on this repository.")
}

/// Registry-view wrapper preserving the original export name.
pub fn isAdvisorBlock(args: &[Value]) -> Value {
    let type_ = args.first().and_then(|v| v.as_str()).unwrap_or("");
    let name = args.get(1).and_then(|v| v.as_str());
    Value::Bool(is_advisor_block(type_, name))
}
