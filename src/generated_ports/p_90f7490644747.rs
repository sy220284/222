//! Native Rust UI structure derived from original: src/components/design-system/ListItem.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"Box".into(),attrs:vec![("paddingLeft".into(),serde_json::json!(2))].into_iter().collect(),children:vec![VNode::Element{tag:"Text".into(),attrs:vec![("color".into(),Value::String("inactive".into()))].into_iter().collect(),children:vec![VNode::Value(Value::Null)]}]}
}
