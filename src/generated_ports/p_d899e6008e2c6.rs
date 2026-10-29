//! Native Rust UI structure derived from original: src/components/SandboxViolationExpandedView.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"Box".into(),attrs:vec![("key".into(),Value::Null),("paddingLeft".into(),serde_json::json!(2))].into_iter().collect(),children:vec![VNode::Element{tag:"Text".into(),attrs:vec![("dimColor".into(),Value::Bool(true))].into_iter().collect(),children:vec![VNode::Value(Value::Null),VNode::Value(if crate::generated_ports::runtime::truthy(&crate::generated_ports::runtime::property(&Value::Null,"command")){Value::Null}else{Value::String("".into())}),VNode::Empty,VNode::Value(crate::generated_ports::runtime::property(&Value::Null,"line"))]}]}
}
