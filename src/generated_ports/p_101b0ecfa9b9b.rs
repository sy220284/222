//! Native Rust UI structure derived from original: src/commands/help/help.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"HelpV2".into(),attrs:vec![("commands".into(),Value::Null),("onClose".into(),Value::Null)].into_iter().collect(),children:vec![]}
}
