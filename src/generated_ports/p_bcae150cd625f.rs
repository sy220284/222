//! Native Rust port derived from original: src/commands/plan/plan.tsx
//!
//! /plan mode-state machine. Entering plan mode runs the transition
//! hook + permission `setMode` update; a non-empty description (other
//! than `open`) requests a follow-up query. Already in plan mode:
//! show the current plan view, "Already in plan mode. No plan written
//! yet." when empty, or `open` routes to the editor flow (editor
//! result handled upstream). Plans/permissions/editor services are
//! injected as plain inputs.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use serde_json::Value;

use crate::generated_ports::ui::VNode;

#[derive(Debug, Clone, PartialEq)]
pub struct PlanCommandOutcome {
    pub message: &'static str,
    pub should_query: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlanViewOutcome {
    /// Mode switch path: permission context update + completion message.
    ModeSwitch(PlanCommandOutcome),
    /// Plan content view (pane + dim path + editor hint).
    Display { content: String, plan_path: String },
    /// No plan written yet while already in plan mode.
    NoPlanYet,
    /// `/plan open` routed to the editor with the plan path.
    OpenEditor(String),
}

/// Pure decision core over injected plan-mode state.
pub fn orchestrate(
    current_mode: &str,
    args: &str,
    plan_content: Option<String>,
    plan_path: String,
) -> PlanViewOutcome {
    if current_mode != "plan" {
        // handlePlanModeTransition + applyPermissionUpdate(setMode,
        // destination session) run in the caller (bootstrap owns them).
        let description = args.trim();
        let should_query = !description.is_empty() && description != "open";
        return PlanViewOutcome::ModeSwitch(PlanCommandOutcome {
            message: "Enabled plan mode",
            should_query,
        });
    }
    let Some(content) = plan_content else {
        return PlanViewOutcome::NoPlanYet;
    };
    if args.trim().split_whitespace().next() == Some("open") {
        return PlanViewOutcome::OpenEditor(plan_path);
    }
    PlanViewOutcome::Display { content, plan_path }
}

/// PlanDisplay pane: bold heading, dim path, boxed content, editor hint
/// only when an editor name resolves (original conditional block).
pub fn render_plan_display(content: &str, plan_path: &str, editor_name: Option<&str>) -> VNode {
    let mut children = vec![
        VNode::Element {
            tag: "Text".into(),
            attrs: [("bold".into(), Value::Bool(true))].into_iter().collect(),
            children: vec![VNode::Text("Current Plan".into())],
        },
        VNode::Element {
            tag: "Text".into(),
            attrs: [("dimColor".into(), Value::Bool(true))]
                .into_iter()
                .collect(),
            children: vec![VNode::Text(plan_path.into())],
        },
        VNode::Element {
            tag: "Box".into(),
            attrs: [("marginTop".into(), Value::from(1))].into_iter().collect(),
            children: vec![VNode::Text(content.into())],
        },
    ];
    if let Some(name) = editor_name {
        if !name.is_empty() {
            children.push(VNode::Element {
                tag: "Box".into(),
                attrs: [("marginTop".into(), Value::from(1))].into_iter().collect(),
                children: vec![VNode::Element {
                    tag: "Text".into(),
                    attrs: [("dimColor".into(), Value::Bool(true))]
                        .into_iter()
                        .collect(),
                    children: vec![VNode::Text(
                        format!("\"/plan open\" to edit this plan in {name}").into(),
                    )],
                }],
            });
        }
    }
    VNode::Element {
        tag: "Box".into(),
        attrs: [("flexDirection".into(), Value::String("column".into()))]
            .into_iter()
            .collect(),
        children,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_plan_mode_with_description_queries() {
        let out = orchestrate("default", "build the login flow", None, "/p/plan".into());
        assert_eq!(
            out,
            PlanViewOutcome::ModeSwitch(PlanCommandOutcome {
                message: "Enabled plan mode",
                should_query: true
            })
        );
    }

    #[test]
    fn enter_plan_mode_bare_or_open_skips_query() {
        let bare = orchestrate("default", "", None, "/p/plan".into());
        assert!(!matches!(
            bare,
            PlanViewOutcome::ModeSwitch(PlanCommandOutcome {
                should_query: true,
                ..
            })
        ));
        let open = orchestrate("default", "open", None, "/p/plan".into());
        assert!(!matches!(
            open,
            PlanViewOutcome::ModeSwitch(PlanCommandOutcome {
                should_query: true,
                ..
            })
        ));
    }

    #[test]
    fn plan_mode_display_open_and_no_plan_paths() {
        assert_eq!(
            orchestrate("plan", "", Some("Do X".into()), "/p/plan".into()),
            PlanViewOutcome::Display {
                content: "Do X".into(),
                plan_path: "/p/plan".into()
            }
        );
        assert_eq!(
            orchestrate("plan", "open", Some("Do X".into()), "/p/plan".into()),
            PlanViewOutcome::OpenEditor("/p/plan".into())
        );
        assert_eq!(
            orchestrate("plan", "", None, "/p/plan".into()),
            PlanViewOutcome::NoPlanYet
        );
    }

    #[test]
    fn display_view_editor_hint_conditional() {
        let with_editor = render_plan_display("Do X", "/p/plan", Some("vim"));
        match with_editor {
            VNode::Element { children, .. } => assert_eq!(children.len(), 4),
            _ => panic!(),
        }
        let no_editor = render_plan_display("Do X", "/p/plan", None);
        match no_editor {
            VNode::Element { children, .. } => assert_eq!(children.len(), 3),
            _ => panic!(),
        }
    }
}
