//! Native Rust UI structure derived from original: src/components/PromptInput/SandboxPromptFooterHint.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"Box".into(),attrs:vec![("paddingX".into(),serde_json::json!(0)),("paddingY".into(),serde_json::json!(0))].into_iter().collect(),children:vec![VNode::Element{tag:"Text".into(),attrs:vec![("color".into(),Value::String("inactive".into())),("wrap".into(),Value::String("truncate".into()))].into_iter().collect(),children:vec![VNode::Text("⧈ Sandbox blocked ".into()),VNode::Value(Value::Null),VNode::Value(Value::String(" ".into())),VNode::Value(Value::Null),VNode::Text("·".into()),VNode::Value(Value::String(" ".into())),VNode::Value(Value::Null),VNode::Text("for details · /sandbox to disable".into())]}]}
}
