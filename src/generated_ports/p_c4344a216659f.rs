//! Native Rust UI structure derived from original: src/commands/install-github-app/WarningsStep.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"Box".into(),attrs:vec![("key".into(),Value::Null),("flexDirection".into(),Value::String("column".into())),("marginBottom".into(),serde_json::json!(1))].into_iter().collect(),children:vec![VNode::Element{tag:"Text".into(),attrs:vec![("color".into(),Value::String("warning".into())),("bold".into(),Value::Bool(true))].into_iter().collect(),children:vec![VNode::Value(crate::generated_ports::runtime::property(&Value::Null,"title"))]},VNode::Element{tag:"Text".into(),attrs:vec![].into_iter().collect(),children:vec![VNode::Value(crate::generated_ports::runtime::property(&Value::Null,"message"))]},VNode::Value(Value::Null)]}
}
