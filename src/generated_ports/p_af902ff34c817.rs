//! Native Rust UI structure derived from original: src/components/TeleportProgress.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"Box".into(),attrs:vec![("key".into(),crate::generated_ports::runtime::property(&Value::Null,"key")),("flexDirection".into(),Value::String("row".into()))].into_iter().collect(),children:vec![VNode::Element{tag:"Box".into(),attrs:vec![("width".into(),serde_json::json!(2))].into_iter().collect(),children:vec![VNode::Element{tag:"Text".into(),attrs:vec![("color".into(),Value::Null),("dimColor".into(),Value::Null)].into_iter().collect(),children:vec![VNode::Value(Value::Null)]}]},VNode::Element{tag:"Text".into(),attrs:vec![("dimColor".into(),Value::Null),("bold".into(),Value::Null)].into_iter().collect(),children:vec![VNode::Value(crate::generated_ports::runtime::property(&Value::Null,"label"))]}]}
}
