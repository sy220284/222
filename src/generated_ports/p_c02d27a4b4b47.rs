//! Native Rust UI structure derived from original: src/components/HelpV2/General.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"Box".into(),attrs:vec![("flexDirection".into(),Value::String("column".into())),("paddingY".into(),serde_json::json!(1)),("gap".into(),serde_json::json!(1))].into_iter().collect(),children:vec![VNode::Value(Value::Null),VNode::Element{tag:"Box".into(),attrs:vec![("flexDirection".into(),Value::String("column".into()))].into_iter().collect(),children:vec![VNode::Element{tag:"Box".into(),attrs:vec![].into_iter().collect(),children:vec![VNode::Element{tag:"Text".into(),attrs:vec![("bold".into(),Value::Bool(true))].into_iter().collect(),children:vec![VNode::Text("Shortcuts".into())]}]},VNode::Element{tag:"PromptInputHelpMenu".into(),attrs:vec![("gap".into(),serde_json::json!(2)),("fixedWidth".into(),Value::Bool(true))].into_iter().collect(),children:vec![]}]}]}
}
