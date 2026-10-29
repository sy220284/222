//! Native Rust UI structure derived from original: src/components/VimTextInput.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"Box".into(),attrs:vec![("flexDirection".into(),Value::String("column".into()))].into_iter().collect(),children:vec![VNode::Element{tag:"BaseTextInput".into(),attrs:vec![("inputState".into(),Value::Null),("terminalFocus".into(),Value::Null),("highlights".into(),crate::generated_ports::runtime::property(&props.clone(),"highlights"))].into_iter().collect(),children:vec![]}]}
}
