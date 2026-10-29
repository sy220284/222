//! Native Rust UI structure derived from original: src/commands/feedback/feedback.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"Feedback".into(),attrs:vec![("abortSignal".into(),Value::Null),("messages".into(),Value::Null),("initialDescription".into(),Value::Null),("onDone".into(),Value::Null),("backgroundTasks".into(),Value::Null)].into_iter().collect(),children:vec![]}
}
