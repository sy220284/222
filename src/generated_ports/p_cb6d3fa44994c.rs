//! Native Rust UI structure derived from original: src/commands/theme/theme.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"Pane".into(),attrs:vec![("color".into(),Value::String("permission".into()))].into_iter().collect(),children:vec![VNode::Element{tag:"ThemePicker".into(),attrs:vec![("onThemeSelect".into(),Value::Null),("onCancel".into(),Value::Null),("skipExitHandling".into(),Value::Bool(true))].into_iter().collect(),children:vec![]}]}
}
