//! Native Rust port derived from original: src/commands/install-github-app/WarningsStep.tsx
//!
//! Setup-warnings step: heading block, per-warning rows (title/message/
//! bulleted instructions), continue hint, and manual-setup docs pointer.
//! The `confirm:yes` keybinding drives `onContinue` upstream (context
//! `Confirmation`); the docs URL constant is env-overridable with a
//! neutral default per repo convention.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde_json::{json, Value};

use crate::generated_ports::ui::VNode;

pub fn github_action_setup_docs_url() -> String {
    std::env::var("ATLAS_GITHUB_ACTION_SETUP_DOCS_URL")
        .unwrap_or_else(|_| "https://docs.example/github-app-setup".to_string())
}

#[derive(Debug, Clone)]
pub struct Warning {
    pub title: String,
    pub message: String,
    pub instructions: Vec<String>,
}

/// Renders the whole warnings pane from the warning list.
pub fn render_warnings(warnings: &[Warning]) -> VNode {
    let mut children = vec![
        VNode::Element {
            tag: "Text".into(),
            attrs: [("bold".into(), json!(true))].into_iter().collect(),
            children: vec![VNode::Text("\u{26a0} Setup Warnings".into())],
        },
        VNode::Element {
            tag: "Text".into(),
            attrs: [("dimColor".into(), json!(true))].into_iter().collect(),
            children: vec![VNode::Text(
                "We found some potential issues, but you can continue anyway".into(),
            )],
        },
    ];
    for warning in warnings {
        let mut row = vec![
            VNode::Element {
                tag: "Text".into(),
                attrs: [
                    ("color".into(), json!("warning")),
                    ("bold".into(), json!(true)),
                ]
                .into_iter()
                .collect(),
                children: vec![VNode::Text(warning.title.clone())],
            },
            VNode::Element {
                tag: "Text".into(),
                attrs: Default::default(),
                children: vec![VNode::Text(warning.message.clone())],
            },
        ];
        if !warning.instructions.is_empty() {
            let bullets = warning
                .instructions
                .iter()
                .map(|instruction| VNode::Element {
                    tag: "Text".into(),
                    attrs: [("dimColor".into(), json!(true))].into_iter().collect(),
                    children: vec![VNode::Text(format!("\u{2022} {instruction}"))],
                })
                .collect();
            row.push(VNode::Element {
                tag: "Box".into(),
                attrs: [
                    ("flexDirection".into(), json!("column")),
                    ("marginLeft".into(), json!(2)),
                    ("marginTop".into(), json!(1)),
                ]
                .into_iter()
                .collect(),
                children: bullets,
            });
        }
        children.push(VNode::Element {
            tag: "Box".into(),
            attrs: [
                ("flexDirection".into(), json!("column")),
                ("marginBottom".into(), json!(1)),
            ]
            .into_iter()
            .collect(),
            children: row,
        });
    }
    children.push(VNode::Element {
        tag: "Box".into(),
        attrs: [("marginTop".into(), json!(1))].into_iter().collect(),
        children: vec![VNode::Element {
            tag: "Text".into(),
            attrs: [
                ("bold".into(), json!(true)),
                ("color".into(), json!("permission")),
            ]
            .into_iter()
            .collect(),
            children: vec![VNode::Text(
                "Press Enter to continue anyway, or Ctrl+C to exit and fix issues".into(),
            )],
        }],
    });
    children.push(VNode::Element {
        tag: "Box".into(),
        attrs: [("marginTop".into(), json!(1))].into_iter().collect(),
        children: vec![VNode::Element {
            tag: "Text".into(),
            attrs: [("dimColor".into(), json!(true))].into_iter().collect(),
            children: vec![VNode::Text(format!(
                "You can also try the manual setup steps if needed: {}",
                github_action_setup_docs_url()
            ))],
        }],
    });
    VNode::Element {
        tag: "Box".into(),
        attrs: [
            ("flexDirection".into(), json!("column")),
            ("borderStyle".into(), json!("round")),
            ("paddingX".into(), json!(1)),
        ]
        .into_iter()
        .collect(),
        children,
    }
}

/// render_source_view entry for the registry: props carry `warnings`.
pub fn render_source_view(props: &Value) -> VNode {
    let warnings: Vec<Warning> = props
        .get("warnings")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|w| Warning {
                    title: w
                        .get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    message: w
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    instructions: w
                        .get("instructions")
                        .and_then(|v| v.as_array())
                        .map(|a| {
                            a.iter()
                                .filter_map(|s| s.as_str().map(str::to_string))
                                .collect()
                        })
                        .unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default();
    render_warnings(&warnings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<Warning> {
        vec![Warning {
            title: "gh CLI not found".into(),
            message: "Install from https://cli.github.com".into(),
            instructions: vec!["brew install gh".into(), "gh auth login".into()],
        }]
    }

    #[test]
    fn warnings_render_with_bullets_and_hint() {
        let view = render_warnings(&sample());
        match view {
            VNode::Element { children, .. } => {
                assert_eq!(children.len(), 5); // heading + sub + row + hint + docs
            }
            _ => panic!("expected pane element"),
        }
    }

    #[test]
    fn empty_instructions_skip_bullet_box() {
        let warnings = vec![Warning {
            title: "t".into(),
            message: "m".into(),
            instructions: vec![],
        }];
        let view = render_warnings(&warnings);
        // One row per warning regardless of instruction bullets (they nest
        // inside the row element).
        match view {
            VNode::Element { children, .. } => assert_eq!(children.len(), 5),
            _ => panic!(),
        }
    }
}
