//! Native Rust UI structure derived from original: src/components/MessageModel.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"Text".into(),attrs:vec![("dimColor".into(),Value::Bool(true))].into_iter().collect(),children:vec![VNode::Value(crate::generated_ports::runtime::property(&crate::generated_ports::runtime::property(&Value::Null,"message"),"model"))]}
}
