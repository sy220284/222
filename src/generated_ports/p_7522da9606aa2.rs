//! Native Rust port derived from original: src/commands/mcp/mcp.tsx
//!
//! /mcp argument routing plus the MCPToggle target-resolution core:
//! the `ide` client is always excluded; `all` scopes to disabled (when
//! enabling) or enabled (when disabling) clients; a named target scopes
//! to that name. Empty selections produce the "already ..."/"not found"
//! messages; successful toggles report counts or per-server outcomes.
//! The toggle action itself is injected; settings/reconnect views are
//! component-domain.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde_json::{json, Value};

use crate::generated_ports::ui::VNode;

#[derive(Debug, Clone, PartialEq)]
pub struct McpClient {
    pub name: String,
    /// "connected" | "disabled" | ... connection type.
    pub ctype: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum McpToggleOutcome {
    AlreadyAll(&'static str),
    NotFound(String),
    ToggledAll { count: usize, enabled: bool },
    ToggledOne { name: String, enabled: bool },
}

/// The original /mcp command always invokes toggleMcpServer(name).
/// For a named target the action label does not change the toggle direction:
/// a currently-disabled server is enabled, every other state is disabled.
/// For "all", resolve_toggle already filtered the corresponding states.
pub fn toggle_enabled_state(action: &str, target: &str, client: &McpClient) -> bool {
    if target == "all" {
        action == "enable"
    } else {
        client.ctype == "disabled"
    }
}

/// MCPToggle core: resolve targets and describe the outcome. Actual
/// toggling is performed by the caller for each returned name.
pub fn resolve_toggle(
    action: &str,
    target: &str,
    clients: &[McpClient],
) -> (McpToggleOutcome, Vec<String>) {
    let is_enabling = action == "enable";
    let candidates: Vec<&McpClient> = clients.iter().filter(|c| c.name != "ide").collect();
    let to_toggle: Vec<&McpClient> = if target == "all" {
        candidates
            .into_iter()
            .filter(|c| {
                if is_enabling {
                    c.ctype == "disabled"
                } else {
                    c.ctype != "disabled"
                }
            })
            .collect()
    } else {
        candidates
            .into_iter()
            .filter(|c| c.name == target)
            .collect()
    };
    if to_toggle.is_empty() {
        return (
            if target == "all" {
                McpToggleOutcome::AlreadyAll(if is_enabling {
                    "All MCP servers are already enabled"
                } else {
                    "All MCP servers are already disabled"
                })
            } else {
                McpToggleOutcome::NotFound(format!("MCP server \"{target}\" not found"))
            },
            vec![],
        );
    }
    let names: Vec<String> = to_toggle.iter().map(|c| c.name.clone()).collect();
    let outcome = if target == "all" {
        McpToggleOutcome::ToggledAll {
            count: names.len(),
            enabled: is_enabling,
        }
    } else {
        McpToggleOutcome::ToggledOne {
            name: target.to_string(),
            enabled: is_enabling,
        }
    };
    (outcome, names)
}

#[derive(Debug, Clone, PartialEq)]
pub enum McpRoute {
    Settings,
    Reconnect(String),
    Toggle { action: String, target: String },
}

/// Argument routing (original `call`).
pub fn route_args(args: Option<&str>) -> McpRoute {
    let Some(args) = args else {
        return McpRoute::Settings;
    };
    let parts: Vec<&str> = args.trim().split_whitespace().collect();
    match parts.first().copied() {
        Some("no-redirect") => McpRoute::Settings,
        Some("reconnect") if parts.len() > 1 => McpRoute::Reconnect(parts[1..].join(" ")),
        Some(a) if a == "enable" || a == "disable" => McpRoute::Toggle {
            action: a.to_string(),
            target: if parts.len() > 1 {
                parts[1..].join(" ")
            } else {
                "all".to_string()
            },
        },
        _ => McpRoute::Settings,
    }
}

pub fn render_settings_view() -> VNode {
    VNode::Element {
        tag: "MCPSettings".into(),
        attrs: [("onComplete".into(), json!(null))].into_iter().collect(),
        children: vec![],
    }
}

pub fn render_reconnect_view(server_name: &str) -> VNode {
    VNode::Element {
        tag: "MCPReconnect".into(),
        attrs: [("serverName".into(), json!(server_name))]
            .into_iter()
            .collect(),
        children: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clients() -> Vec<McpClient> {
        vec![
            McpClient {
                name: "a".into(),
                ctype: "connected".into(),
            },
            McpClient {
                name: "b".into(),
                ctype: "disabled".into(),
            },
            McpClient {
                name: "ide".into(),
                ctype: "connected".into(),
            },
        ]
    }

    #[test]
    fn enable_all_scopes_to_disabled_excluding_ide() {
        let (outcome, names) = resolve_toggle("enable", "all", &clients());
        assert_eq!(
            outcome,
            McpToggleOutcome::ToggledAll {
                count: 1,
                enabled: true
            }
        );
        assert_eq!(names, vec!["b"]);
    }

    #[test]
    fn disable_all_scopes_to_non_disabled() {
        let (outcome, names) = resolve_toggle("disable", "all", &clients());
        assert_eq!(
            outcome,
            McpToggleOutcome::ToggledAll {
                count: 1,
                enabled: false
            }
        );
        assert_eq!(names, vec!["a"]);
    }

    #[test]
    fn named_target_and_not_found() {
        let (outcome, names) = resolve_toggle("enable", "a", &clients());
        assert_eq!(
            outcome,
            McpToggleOutcome::ToggledOne {
                name: "a".into(),
                enabled: true
            }
        );
        assert_eq!(names, vec!["a"]);
        let (missing, _) = resolve_toggle("enable", "zzz", &clients());
        assert_eq!(
            missing,
            McpToggleOutcome::NotFound("MCP server \"zzz\" not found".into())
        );
        let already = resolve_toggle(
            "enable",
            "all",
            &[McpClient {
                name: "a".into(),
                ctype: "connected".into(),
            }],
        );
        assert_eq!(
            already.0,
            McpToggleOutcome::AlreadyAll("All MCP servers are already enabled")
        );
    }

    #[test]
    fn named_toggle_follows_live_state_even_when_action_label_differs() {
        let active = McpClient {
            name: "a".into(),
            ctype: "connected".into(),
        };
        let disabled = McpClient {
            name: "a".into(),
            ctype: "disabled".into(),
        };
        assert!(!toggle_enabled_state("enable", "a", &active));
        assert!(!toggle_enabled_state("disable", "a", &active));
        assert!(toggle_enabled_state("enable", "a", &disabled));
        assert!(toggle_enabled_state("disable", "a", &disabled));
        assert!(toggle_enabled_state("enable", "all", &active));
        assert!(!toggle_enabled_state("disable", "all", &disabled));
    }

    #[test]
    fn args_route() {
        assert_eq!(route_args(None), McpRoute::Settings);
        assert_eq!(route_args(Some("no-redirect")), McpRoute::Settings);
        assert_eq!(
            route_args(Some("reconnect my server")),
            McpRoute::Reconnect("my server".into())
        );
        assert_eq!(
            route_args(Some("disable b")),
            McpRoute::Toggle {
                action: "disable".into(),
                target: "b".into()
            }
        );
        assert_eq!(
            route_args(Some("enable")),
            McpRoute::Toggle {
                action: "enable".into(),
                target: "all".into()
            }
        );
    }

    #[test]
    fn views_carry_props() {
        match render_reconnect_view("srv") {
            VNode::Element { tag, attrs, .. } => {
                assert_eq!(tag, "MCPReconnect");
                assert_eq!(attrs.get("serverName"), Some(&json!("srv")));
            }
            _ => panic!(),
        }
    }
}
