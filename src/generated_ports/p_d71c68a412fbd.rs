//! Native Rust port derived from original: src/commands/sandbox-toggle/sandbox-toggle.tsx
//!
//! /sandbox orchestration: platform-support gate -> dependency check ->
//! enterprise enabledPlatforms list -> policy lock -> argument routing
//! (empty = settings menu, `exclude <pattern>` = settings write with
//! cwd-relative path, unknown = error). SandboxManager and the settings
//! loader are injected as plain data; the settings-menu view remains a
//! component-domain VNode.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde_json::{json, Value};

use crate::generated_ports::ui::VNode;

pub struct SandboxGuards {
    pub supported_platform: bool,
    pub platform: &'static str,
    pub platform_in_enabled_list: bool,
    pub settings_locked_by_policy: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SandboxOutcome {
    Error(String),
    /// Interactive settings menu (component-domain view).
    Menu,
    Excluded(String),
}

pub fn error_message(platform: &str, theme_name: &str) -> String {
    if platform == "wsl" {
        format!("\u{1b}[31mError: Sandboxing requires WSL2. WSL1 is not supported.\u{1b}[0m")
    } else if platform == "macos" {
        format!("\u{1b}[32mok:{theme_name}\u{1b}[0m")
    } else {
        format!(
            "\u{1b}[31mError: Sandboxing is currently only supported on macOS, Linux, and WSL2.\u{1b}[0m"
        )
    }
}

/// `exclude <pattern>` handling: quote stripping, settings write note with
/// cwd-relative path (fixed fallback when the settings file path is absent).
pub fn exclude_outcome(
    pattern_raw: &str,
    cwd: &str,
    settings_path: Option<&str>,
) -> Result<String, String> {
    let command_pattern = pattern_raw.trim();
    if command_pattern.is_empty() {
        return Err(
            "Error: Please provide a command pattern to exclude (e.g., /sandbox exclude \"npm run test:*\")"
                .to_string(),
        );
    }
    let clean = command_pattern
        .trim_start_matches(['"', '\''])
        .trim_end_matches(['"', '\'']);
    let relative_path = settings_path
        .and_then(|p| pathdiff_relative(cwd, p))
        .unwrap_or_else(|| ".atlas/settings.local.json".to_string());
    Ok(format!(
        "Added \"{clean}\" to excluded commands in {relative_path}"
    ))
}

/// Minimal relative-path (same-prefix walk) for the settings note.
fn pathdiff_relative(cwd: &str, target: &str) -> Option<String> {
    let cwd_parts: Vec<&str> = cwd.split('/').filter(|s| !s.is_empty()).collect();
    let tgt_parts: Vec<&str> = target.split('/').filter(|s| !s.is_empty()).collect();
    let mut i = 0;
    while i < cwd_parts.len() && i < tgt_parts.len() && cwd_parts[i] == tgt_parts[i] {
        i += 1;
    }
    if i == 0 {
        return Some(target.to_string());
    }
    let mut out: Vec<String> = Vec::new();
    for _ in i..cwd_parts.len() {
        out.push("..".to_string());
    }
    for part in &tgt_parts[i..] {
        out.push((*part).to_string());
    }
    Some(out.join("/"))
}

/// The full command routing, mirroring the original guard order.
pub fn orchestrate(
    guards: &SandboxGuards,
    args: Option<&str>,
    cwd: &str,
    settings_path: Option<&str>,
) -> SandboxOutcome {
    if !guards.supported_platform {
        return SandboxOutcome::Error(error_message(guards.platform, "light"));
    }
    if !guards.platform_in_enabled_list {
        return SandboxOutcome::Error(format!(
            "Error: Sandboxing is disabled for this platform ({}) via the enabledPlatforms setting.",
            guards.platform
        ));
    }
    if guards.settings_locked_by_policy {
        return SandboxOutcome::Error(
            "Error: Sandbox settings are overridden by a higher-priority configuration and cannot be changed locally."
                .to_string(),
        );
    }
    let trimmed = args.unwrap_or("").trim();
    if trimmed.is_empty() {
        return SandboxOutcome::Menu;
    }
    let mut parts = trimmed.splitn(2, ' ');
    let subcommand = parts.next().unwrap_or("");
    if subcommand != "exclude" {
        return SandboxOutcome::Error(format!(
            "Error: Unknown subcommand \"{subcommand}\". Available subcommand: exclude"
        ));
    }
    match exclude_outcome(parts.next().unwrap_or(""), cwd, settings_path) {
        Ok(msg) => SandboxOutcome::Excluded(msg),
        Err(msg) => SandboxOutcome::Error(msg),
    }
}

/// Settings-menu view (component-domain: SandboxSettings with depCheck).
pub fn render_menu(dep_check: &Value) -> VNode {
    VNode::Element {
        tag: "SandboxSettings".into(),
        attrs: [("depCheck".into(), dep_check.clone())]
            .into_iter()
            .collect(),
        children: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_platform_gate_wsl_vs_other() {
        let guards = SandboxGuards {
            supported_platform: false,
            platform: "wsl",
            platform_in_enabled_list: true,
            settings_locked_by_policy: false,
        };
        match orchestrate(&guards, Some("exclude x"), "/", None) {
            SandboxOutcome::Error(msg) => assert!(msg.contains("WSL2"), "{msg}"),
            other => panic!("expected error, got {:?}", other),
        }
        let guards2 = SandboxGuards {
            supported_platform: false,
            platform: "linux",
            ..guards
        };
        match orchestrate(&guards2, None, "/", None) {
            SandboxOutcome::Error(msg) => assert!(msg.contains("only supported on macOS"), "{msg}"),
            other => panic!("expected error, got {:?}", other),
        }
    }

    #[test]
    fn enabled_list_and_policy_lock_precede_args() {
        let guards = SandboxGuards {
            supported_platform: true,
            platform: "linux",
            platform_in_enabled_list: false,
            settings_locked_by_policy: false,
        };
        match orchestrate(&guards, None, "/", None) {
            SandboxOutcome::Error(msg) => assert!(msg.contains("enabledPlatforms"), "{msg}"),
            other => panic!("expected error, got {:?}", other),
        }
        let locked = SandboxGuards {
            platform_in_enabled_list: true,
            settings_locked_by_policy: true,
            ..guards
        };
        match orchestrate(&locked, Some("exclude x"), "/", None) {
            SandboxOutcome::Error(msg) => assert!(msg.contains("higher-priority"), "{msg}"),
            other => panic!("expected error, got {:?}", other),
        }
    }

    #[test]
    fn empty_args_render_menu_with_depcheck() {
        let guards = SandboxGuards {
            supported_platform: true,
            platform: "macos",
            platform_in_enabled_list: true,
            settings_locked_by_policy: false,
        };
        match orchestrate(&guards, None, "/", None) {
            SandboxOutcome::Menu => {}
            other => panic!("expected menu, got {:?}", other),
        }
    }

    #[test]
    fn exclude_strips_quotes_and_reports_relative_path() {
        let guards = SandboxGuards {
            supported_platform: true,
            platform: "macos",
            platform_in_enabled_list: true,
            settings_locked_by_policy: false,
        };
        let cwd = "/home/u/proj";
        let sp = "/home/u/proj/.atlas/settings.local.json";
        match orchestrate(&guards, Some("exclude \"npm run test:*\""), cwd, Some(sp)) {
            SandboxOutcome::Excluded(msg) => {
                assert!(
                    msg.contains("Added \"npm run test:*\" to excluded commands"),
                    "{msg}"
                );
                assert!(msg.contains(".atlas/settings.local.json"), "{msg}");
            }
            other => panic!("expected excluded, got {:?}", other),
        }
        // Missing pattern errors.
        match orchestrate(&guards, Some("exclude"), cwd, None) {
            SandboxOutcome::Error(msg) => {
                assert!(msg.contains("provide a command pattern"), "{msg}")
            }
            other => panic!("expected error, got {:?}", other),
        }
        // Unknown subcommand.
        match orchestrate(&guards, Some("frobnicate x"), cwd, None) {
            SandboxOutcome::Error(msg) => assert!(msg.contains("Unknown subcommand"), "{msg}"),
            other => panic!("expected error, got {:?}", other),
        }
    }

    #[test]
    fn menu_view_carries_depcheck() {
        let view = render_menu(&json!({"errors": [], "warnings": []}));
        match view {
            VNode::Element { attrs, .. } => assert!(attrs.contains_key("depCheck")),
            _ => panic!("expected element"),
        }
    }
}
