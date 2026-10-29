//! Native Rust UI structure derived from original: src/components/TaskListV2.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"Box".into(),attrs:vec![("flexDirection".into(),Value::String("column".into())),("marginTop".into(),serde_json::json!(1)),("marginLeft".into(),serde_json::json!(2))].into_iter().collect(),children:vec![VNode::Empty,VNode::Element{tag:"Box".into(),attrs:vec![].into_iter().collect(),children:vec![VNode::Empty,VNode::Element{tag:"Text".into(),attrs:vec![("dimColor".into(),Value::Bool(true))].into_iter().collect(),children:vec![VNode::Empty,VNode::Element{tag:"Text".into(),attrs:vec![("bold".into(),Value::Bool(true))].into_iter().collect(),children:vec![VNode::Value(crate::generated_ports::runtime::property(&Value::Null,"length"))]},VNode::Empty,VNode::Value(Value::String(" tasks (".into())),VNode::Empty,VNode::Element{tag:"Text".into(),attrs:vec![("bold".into(),Value::Bool(true))].into_iter().collect(),children:vec![VNode::Value(Value::Null)]},VNode::Empty,VNode::Value(Value::String(" done, ".into())),VNode::Empty,VNode::Value(Value::Null),VNode::Empty,VNode::Element{tag:"Text".into(),attrs:vec![("bold".into(),Value::Bool(true))].into_iter().collect(),children:vec![VNode::Value(Value::Null)]},VNode::Empty,VNode::Value(Value::String(" open)".into())),VNode::Empty]},VNode::Empty]},VNode::Empty,VNode::Value(Value::Null),VNode::Empty]}
}
