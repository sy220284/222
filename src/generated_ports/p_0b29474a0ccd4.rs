//! Native Rust UI structure derived from original: src/components/BashModeProgress.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"ShellProgressMessage".into(),attrs:vec![("fullOutput".into(),crate::generated_ports::runtime::property(&Value::Null,"fullOutput")),("output".into(),crate::generated_ports::runtime::property(&Value::Null,"output")),("elapsedTimeSeconds".into(),crate::generated_ports::runtime::property(&Value::Null,"elapsedTimeSeconds")),("totalLines".into(),crate::generated_ports::runtime::property(&Value::Null,"totalLines")),("verbose".into(),Value::Null)].into_iter().collect(),children:vec![]}
}
