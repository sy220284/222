//! Native Rust UI structure derived from original: src/components/LogoV2/CondensedLogo.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"OffscreenFreeze".into(),attrs:vec![].into_iter().collect(),children:vec![VNode::Element{tag:"Box".into(),attrs:vec![("flexDirection".into(),Value::String("row".into())),("gap".into(),serde_json::json!(2)),("alignItems".into(),Value::String("center".into()))].into_iter().collect(),children:vec![VNode::Value(Value::Null),VNode::Element{tag:"Box".into(),attrs:vec![("flexDirection".into(),Value::String("column".into()))].into_iter().collect(),children:vec![VNode::Value(Value::Null),VNode::Value(Value::Null),VNode::Value(Value::Null),VNode::Value(Value::Null),VNode::Value(Value::Null)]}]}]}
}
