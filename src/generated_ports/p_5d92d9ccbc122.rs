//! Native Rust UI structure derived from original: src/components/ValidationErrorsList.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"Box".into(),attrs:vec![("key".into(),Value::Null),("flexDirection".into(),Value::String("column".into()))].into_iter().collect(),children:vec![VNode::Element{tag:"Text".into(),attrs:vec![].into_iter().collect(),children:vec![VNode::Value(Value::Null)]},VNode::Element{tag:"Box".into(),attrs:vec![("marginLeft".into(),serde_json::json!(1))].into_iter().collect(),children:vec![VNode::Element{tag:"Text".into(),attrs:vec![("dimColor".into(),Value::Bool(true))].into_iter().collect(),children:vec![VNode::Value(Value::Null)]}]},VNode::Value(Value::Null)]}
}
