//! Native Rust port derived from original: src/commands/plugin/PluginOptionsFlow.tsx
//!
//! Post-install config walk. Two pure cores: the just-installed target
//! lookup (matches `repository` OR `source` against the pluginId across
//! enabled+disabled lists) and the step-list builder (top-level
//! manifest.userConfig first, then per-channel; empty steps mean
//! "skipped" immediately). Saving dispatches to the top-level or
//! channel-specific storage path; `error` maps any save throw. The
//! PluginOptionsDialog widget itself is component-domain.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct LoadedPlugin {
    pub name: String,
    pub repository: String,
    pub source: String,
}

/// Original `findPluginOptionsTarget`: first match by repository or source.
pub fn find_plugin_options_target(
    plugin_id: &str,
    enabled: &[LoadedPlugin],
    disabled: &[LoadedPlugin],
) -> Option<LoadedPlugin> {
    enabled
        .iter()
        .chain(disabled.iter())
        .find(|p| p.repository == plugin_id || p.source == plugin_id)
        .cloned()
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigStepSpec {
    /// "top-level" or the channel name.
    pub key: String,
    pub title: String,
    pub subtitle: String,
    /// Unconfigured option schema (flat JSON object).
    pub schema: Value,
}

/// Step-list builder (original mount-time `steps`). `top_unconfigured` is
/// the already-computed `getUnconfiguredOptions` result; `channels` is the
/// already-computed unconfigured channel list (name + prompt strings).
pub fn build_config_steps(
    plugin_name: &str,
    top_unconfigured: &Value,
    channels: &[(String, String)],
) -> Vec<ConfigStepSpec> {
    let mut steps = Vec::new();
    let top_keys = top_unconfigured.as_object().map(|m| m.len()).unwrap_or(0);
    if top_keys > 0 {
        steps.push(ConfigStepSpec {
            key: "top-level".into(),
            title: format!("Configure {plugin_name}"),
            subtitle: "Plugin options".into(),
            schema: top_unconfigured.clone(),
        });
    }
    for (name, prompt) in channels {
        steps.push(ConfigStepSpec {
            key: name.clone(),
            title: format!("Configure {name}"),
            subtitle: prompt.clone(),
            schema: json!({}),
        });
    }
    steps
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowOutcome {
    Configured,
    Skipped,
    Error,
}

/// onDone mapping for a dialog step save: any error -> `error`, otherwise
/// the walk continues; a finished walk with zero remaining steps is
/// `configured`, a zero-length step list at entry is `skipped`.
pub fn outcome_for(steps_remaining: usize, save_error: Option<&str>) -> FlowOutcome {
    if save_error.is_some() {
        FlowOutcome::Error
    } else if steps_remaining == 0 {
        FlowOutcome::Configured
    } else {
        FlowOutcome::Skipped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(name: &str, repo: &str, source: &str) -> LoadedPlugin {
        LoadedPlugin {
            name: name.into(),
            repository: repo.into(),
            source: source.into(),
        }
    }

    #[test]
    fn target_lookup_matches_repository_or_source_across_lists() {
        let enabled = vec![p("a", "repo/a", "src/a")];
        let disabled = vec![p("b", "repo/b", "src/b")];
        assert_eq!(
            find_plugin_options_target("repo/b", &enabled, &disabled).unwrap(),
            p("b", "repo/b", "src/b")
        );
        assert_eq!(
            find_plugin_options_target("src/a", &enabled, &disabled).unwrap(),
            p("a", "repo/a", "src/a")
        );
        assert!(find_plugin_options_target("nope", &enabled, &disabled).is_none());
    }

    #[test]
    fn steps_top_level_first_then_channels() {
        let steps = build_config_steps(
            "my-tool",
            &json!({"apiKey": {"type": "string"}}),
            &[("mcp-server".into(), "Needs an API key".into())],
        );
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].key, "top-level");
        assert_eq!(steps[0].title, "Configure my-tool");
        assert_eq!(steps[1].key, "mcp-server");
        // Fully configured top-level collapses the step.
        let none = build_config_steps("my-tool", &json!({}), &[]);
        assert!(none.is_empty());
    }

    #[test]
    fn outcome_semantics_match_on_done_contract() {
        assert_eq!(outcome_for(0, None), FlowOutcome::Configured);
        assert_eq!(outcome_for(3, None), FlowOutcome::Skipped);
        assert_eq!(outcome_for(1, Some("disk full")), FlowOutcome::Error);
    }
}
