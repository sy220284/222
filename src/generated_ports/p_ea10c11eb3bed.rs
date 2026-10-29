//! Native Rust UI structure derived from original: src/components/LogoV2/WelcomeV2.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"Text".into(),attrs:vec![].into_iter().collect(),children:vec![VNode::Value(Value::String("…………………".into())),VNode::Element{tag:"Text".into(),attrs:vec![("backgroundColor".into(),Value::String("clawd_body".into()))].into_iter().collect(),children:vec![VNode::Empty]},VNode::Element{tag:"Text".into(),attrs:vec![].into_iter().collect(),children:vec![VNode::Empty]},VNode::Element{tag:"Text".into(),attrs:vec![("backgroundColor".into(),Value::String("clawd_body".into()))].into_iter().collect(),children:vec![VNode::Empty]},VNode::Element{tag:"Text".into(),attrs:vec![].into_iter().collect(),children:vec![VNode::Value(Value::String("   ".into()))]},VNode::Element{tag:"Text".into(),attrs:vec![("backgroundColor".into(),Value::String("clawd_body".into()))].into_iter().collect(),children:vec![VNode::Empty]},VNode::Element{tag:"Text".into(),attrs:vec![].into_iter().collect(),children:vec![VNode::Empty]},VNode::Element{tag:"Text".into(),attrs:vec![("backgroundColor".into(),Value::String("clawd_body".into()))].into_iter().collect(),children:vec![VNode::Empty]},VNode::Value(Value::String("……………………………………………………………………░…………………………▒…………".into()))]}
}
