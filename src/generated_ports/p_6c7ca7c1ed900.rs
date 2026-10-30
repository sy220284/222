//! Native Rust port derived from original: src/commands/plugin/pluginDetailsHelpers.tsx
//!
//! Shared helpers for plugin details views (DiscoverPlugins and
//! BrowseMarketplace): GitHub repo extraction from a plugin source,
//! scoped-install menu construction (user/project/local always,
//! homepage/github conditional, back last). Views are thin wrappers.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde_json::{json, Value};

use crate::generated_ports::ui::VNode;

#[derive(Debug, Clone, PartialEq)]
pub struct InstallablePlugin {
    pub entry: Value,
    pub marketplace_name: String,
    pub plugin_id: String,
    pub is_installed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginDetailsMenuOption {
    pub label: String,
    pub action: String,
}

/// Extract GitHub repo info from a plugin entry source
/// (`source: { source: "github", repo }`).
pub fn extract_github_repo(plugin: &InstallablePlugin) -> Option<String> {
    let source = plugin.entry.get("source")?;
    if source.get("source").and_then(|v| v.as_str()) == Some("github") {
        source
            .get("repo")
            .and_then(|v| v.as_str())
            .map(str::to_owned)
    } else {
        None
    }
}

/// Menu options with scoped installation options (original order).
pub fn build_plugin_details_menu_options(
    has_homepage: Option<&str>,
    github_repo: Option<&str>,
) -> Vec<PluginDetailsMenuOption> {
    let mut options = vec![
        PluginDetailsMenuOption {
            label: "Install for you (user scope)".into(),
            action: "install-user".into(),
        },
        PluginDetailsMenuOption {
            label: "Install for all collaborators on this repository (project scope)".into(),
            action: "install-project".into(),
        },
        PluginDetailsMenuOption {
            label: "Install for you, in this repo only (local scope)".into(),
            action: "install-local".into(),
        },
    ];
    if has_homepage.is_some() {
        options.push(PluginDetailsMenuOption {
            label: "Open homepage".into(),
            action: "homepage".into(),
        });
    }
    if github_repo.is_some() {
        options.push(PluginDetailsMenuOption {
            label: "View on GitHub".into(),
            action: "github".into(),
        });
    }
    options.push(PluginDetailsMenuOption {
        label: "Back to plugin list".into(),
        action: "back".into(),
    });
    options
}

/// Key hint strip for plugin selection screens.
pub fn render_selection_key_hint() -> VNode {
    VNode::Element {
        tag: "Box".into(),
        attrs: [("gap".into(), json!(2))].into_iter().collect(),
        children: vec![
            VNode::Element {
                tag: "Text".into(),
                attrs: [("dimColor".into(), json!(true))].into_iter().collect(),
                children: vec![VNode::Text("Enter to select".into())],
            },
            VNode::Element {
                tag: "Text".into(),
                attrs: [("dimColor".into(), json!(true))].into_iter().collect(),
                children: vec![VNode::Text("Esc to go back".into())],
            },
        ],
    }
}

/// Byline strip for plugin details (author + version).
pub fn render_byline(author: &str, version: &str) -> VNode {
    VNode::Element {
        tag: "Text".into(),
        attrs: [("dimColor".into(), json!(true))].into_iter().collect(),
        children: vec![VNode::Text(format!("{author} · v{version}"))],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn github_repo_extraction() {
        let plugin = InstallablePlugin {
            entry: json!({"source": {"source": "github", "repo": "o/r"}}),
            marketplace_name: "m".into(),
            plugin_id: "m:p".into(),
            is_installed: false,
        };
        assert_eq!(extract_github_repo(&plugin).as_deref(), Some("o/r"));
        let non_gh = InstallablePlugin {
            entry: json!({"source": {"source": "url", "url": "https://x"}}),
            ..plugin
        };
        assert_eq!(extract_github_repo(&non_gh), None);
    }

    #[test]
    fn menu_options_scoped_and_conditional() {
        let base = build_plugin_details_menu_options(None, None);
        assert_eq!(base.len(), 4);
        assert_eq!(base[0].action, "install-user");
        assert_eq!(base.last().unwrap().action, "back");
        let full = build_plugin_details_menu_options(Some("https://home"), Some("o/r"));
        assert_eq!(full.len(), 6);
        assert_eq!(full[3].action, "homepage");
        assert_eq!(full[4].action, "github");
    }

    #[test]
    fn hint_and_byline_views() {
        match render_selection_key_hint() {
            VNode::Element { children, .. } => assert_eq!(children.len(), 2),
            _ => panic!(),
        }
        match render_byline("alice", "1.2.0") {
            VNode::Element { children, .. } => match &children[0] {
                VNode::Text(t) => assert_eq!(t, "alice · v1.2.0"),
                _ => panic!(),
            },
            _ => panic!(),
        }
    }
}
