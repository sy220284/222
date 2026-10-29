//! Native Rust UI structure derived from original: src/components/agents/new-agent-creation/wizard-steps/ConfirmStep.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"Byline".into(),attrs:vec![].into_iter().collect(),children:vec![VNode::Element{tag:"KeyboardShortcutHint".into(),attrs:vec![("shortcut".into(),Value::String("s/Enter".into())),("action".into(),Value::String("save".into()))].into_iter().collect(),children:vec![]},VNode::Element{tag:"KeyboardShortcutHint".into(),attrs:vec![("shortcut".into(),Value::String("e".into())),("action".into(),Value::String("edit in your editor".into()))].into_iter().collect(),children:vec![]},VNode::Element{tag:"ConfigurableShortcutHint".into(),attrs:vec![("action".into(),Value::String("confirm:no".into())),("context".into(),Value::String("Confirmation".into())),("fallback".into(),Value::String("Esc".into())),("description".into(),Value::String("cancel".into()))].into_iter().collect(),children:vec![]}]}
}
