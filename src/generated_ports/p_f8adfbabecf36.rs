//! Native Rust UI structure derived from original: src/components/OutputStylePicker.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"Box".into(),attrs:vec![("flexDirection".into(),Value::String("column".into())),("gap".into(),serde_json::json!(1))].into_iter().collect(),children:vec![VNode::Value(Value::Null),VNode::Value(if crate::generated_ports::runtime::truthy(&Value::Null){Value::Null}else{Value::Null})]}
}
