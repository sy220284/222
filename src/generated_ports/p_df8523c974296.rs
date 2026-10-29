//! Native Rust UI structure derived from original: src/components/MessageResponse.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"MessageResponseProvider".into(),attrs:vec![].into_iter().collect(),children:vec![VNode::Element{tag:"Box".into(),attrs:vec![("flexDirection".into(),Value::String("row".into())),("height".into(),Value::Null),("overflowY".into(),Value::String("hidden".into()))].into_iter().collect(),children:vec![VNode::Value(Value::Null),VNode::Value(Value::Null)]}]}
}
