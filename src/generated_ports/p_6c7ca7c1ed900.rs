//! Native Rust UI structure derived from original: src/commands/plugin/pluginDetailsHelpers.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"ConfigurableShortcutHint".into(),attrs:vec![("action".into(),Value::String("plugin:install".into())),("context".into(),Value::String("Plugin".into())),("fallback".into(),Value::String("i".into())),("description".into(),Value::String("install".into())),("bold".into(),Value::Bool(true))].into_iter().collect(),children:vec![]}
}
