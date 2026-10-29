//! Native Rust UI structure derived from original: src/components/agents/new-agent-creation/wizard-steps/TypeStep.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"Box".into(),attrs:vec![("marginTop".into(),serde_json::json!(1))].into_iter().collect(),children:vec![VNode::Element{tag:"TextInput".into(),attrs:vec![("value".into(),Value::Null),("onChange".into(),Value::Null),("onSubmit".into(),Value::Null),("placeholder".into(),Value::String("e.g., test-runner, tech-lead, etc".into())),("columns".into(),serde_json::json!(60)),("cursorOffset".into(),Value::Null),("onChangeCursorOffset".into(),Value::Null),("focus".into(),Value::Bool(true)),("showCursor".into(),Value::Bool(true))].into_iter().collect(),children:vec![]}]}
}
