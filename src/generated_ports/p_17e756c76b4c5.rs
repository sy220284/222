//! Native Rust UI structure derived from original: src/components/design-system/LoadingState.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"Box".into(),attrs:vec![("flexDirection".into(),Value::String("row".into()))].into_iter().collect(),children:vec![VNode::Value(Value::Null),VNode::Element{tag:"Text".into(),attrs:vec![("bold".into(),Value::Null),("dimColor".into(),Value::Null)].into_iter().collect(),children:vec![VNode::Value(Value::String(" ".into())),VNode::Value(Value::Null)]}]}
}
