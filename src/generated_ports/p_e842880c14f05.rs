//! Rust exports derived from original: src/components/ChannelDowngradeDialog.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
pub enum ChannelDowngradeChoice{
    #[serde(rename="downgrade")]
    downgrade,
    #[serde(rename="stay")]
    stay,
    #[serde(rename="cancel")]
    cancel,
}

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"Dialog".into(),attrs:vec![("title".into(),Value::String("Switch to Stable Channel".into())),("onCancel".into(),Value::Null),("color".into(),Value::String("permission".into())),("hideBorder".into(),Value::Bool(true)),("hideInputGuide".into(),Value::Bool(true))].into_iter().collect(),children:vec![VNode::Value(Value::Null),VNode::Value(Value::Null),VNode::Value(Value::Null)]}
}
