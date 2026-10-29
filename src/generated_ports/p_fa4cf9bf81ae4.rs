//! Native Rust UI structure derived from original: src/commands/extra-usage/extra-usage.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"Login".into(),attrs:vec![("startingMessage".into(),Value::String("Starting new login following /extra-usage. Exit with Ctrl-C to use existing account.".into())),("onDone".into(),Value::Null)].into_iter().collect(),children:vec![]}
}
