//! Native Rust UI structure derived from original: src/commands/skills/skills.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"SkillsMenu".into(),attrs:vec![("onExit".into(),Value::Null),("commands".into(),crate::generated_ports::runtime::property(&crate::generated_ports::runtime::property(&Value::Null,"options"),"commands"))].into_iter().collect(),children:vec![]}
}
