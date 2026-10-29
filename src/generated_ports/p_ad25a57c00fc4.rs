//! Native Rust UI structure derived from original: src/components/HelpV2/HelpV2.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"Tab".into(),attrs:vec![("key".into(),Value::String("custom".into())),("title".into(),Value::String("custom-commands".into()))].into_iter().collect(),children:vec![VNode::Element{tag:"Commands".into(),attrs:vec![("commands".into(),Value::Null),("maxHeight".into(),Value::Null),("columns".into(),Value::Null),("title".into(),Value::String("Browse custom commands:".into())),("emptyMessage".into(),Value::String("No custom commands found".into())),("onCancel".into(),Value::Null)].into_iter().collect(),children:vec![]}]}
}
