//! Native Rust port derived from original: src/commands/theme/theme.tsx
//!
//! /theme command: opens the theme picker inside a permission-colored
//! pane. Selection applies the theme and completes with
//! "Theme set to <setting>"; dismissal completes with
//! "Theme picker dismissed" and a `system` display hint. The picker
//! widget itself lives in the components domain; this port carries the
//! command's contract (callbacks, completion payloads, skipExitHandling)
//! and drives the pane view.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde_json::{json, Value};

use crate::generated_ports::ui::VNode;

/// Outcome of the theme command, mirroring the two `onDone` paths.
#[derive(Debug, Clone, PartialEq)]
pub enum ThemeCommandOutcome {
    /// `Theme set to <setting>` — a theme was chosen.
    Applied(String),
    /// `Theme picker dismissed` with `display: system`.
    Dismissed,
}

impl ThemeCommandOutcome {
    pub fn message(&self) -> String {
        match self {
            ThemeCommandOutcome::Applied(setting) => format!("Theme set to {setting}"),
            ThemeCommandOutcome::Dismissed => "Theme picker dismissed".to_string(),
        }
    }

    pub fn display(&self) -> Option<&str> {
        match self {
            ThemeCommandOutcome::Applied(_) => None,
            ThemeCommandOutcome::Dismissed => Some("system"),
        }
    }
}

/// Pure reducer: what the command does when the picker reports a choice.
pub fn on_theme_select(setting: &str) -> ThemeCommandOutcome {
    ThemeCommandOutcome::Applied(setting.to_string())
}

/// Pure reducer: what the command does when the picker is cancelled.
pub fn on_theme_cancel() -> ThemeCommandOutcome {
    ThemeCommandOutcome::Dismissed
}

/// Pane view: permission-colored container holding the picker widget.
pub fn render_source_view(props: &Value) -> VNode {
    let skip_exit_handling = props
        .get("skipExitHandling")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    VNode::Element {
        tag: "Pane".into(),
        attrs: vec![("color".into(), Value::String("permission".into()))]
            .into_iter()
            .collect(),
        children: vec![VNode::Element {
            tag: "ThemePicker".into(),
            attrs: vec![
                (
                    "onThemeSelect".into(),
                    json!("setting => setTheme(setting)"),
                ),
                ("onCancel".into(), json!("() => onDone(dismissed)")),
                ("skipExitHandling".into(), json!(skip_exit_handling)),
            ]
            .into_iter()
            .collect(),
            children: vec![],
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_outcome_message_and_display() {
        let outcome = on_theme_select("dark");
        assert_eq!(outcome.message(), "Theme set to dark");
        assert_eq!(outcome.display(), None);
    }

    #[test]
    fn cancel_outcome_carries_system_display() {
        let outcome = on_theme_cancel();
        assert_eq!(outcome.message(), "Theme picker dismissed");
        assert_eq!(outcome.display(), Some("system"));
    }

    #[test]
    fn pane_view_wraps_picker_with_permission_color() {
        let view = render_source_view(&json!({}));
        match view {
            VNode::Element {
                tag,
                attrs,
                children,
            } => {
                assert_eq!(tag, "Pane");
                assert!(attrs
                    .iter()
                    .any(|(k, v)| k == "color" && v == &json!("permission")));
                assert_eq!(children.len(), 1);
            }
            _ => panic!("expected element vnode"),
        }
    }
}
