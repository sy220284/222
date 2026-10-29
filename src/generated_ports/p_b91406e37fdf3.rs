//! Native Rust UI structure derived from original: src/components/agents/ColorPicker.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"Box".into(),attrs:vec![("key".into(),Value::Null),("flexDirection".into(),Value::String("row".into())),("gap".into(),serde_json::json!(1))].into_iter().collect(),children:vec![VNode::Element{tag:"Text".into(),attrs:vec![("color".into(),if crate::generated_ports::runtime::truthy(&Value::Null){Value::String("suggestion".into())}else{Value::Null})].into_iter().collect(),children:vec![VNode::Value(if crate::generated_ports::runtime::truthy(&Value::Null){crate::generated_ports::runtime::property(&Value::Null,"pointer")}else{Value::String(" ".into())})]},VNode::Value(if crate::generated_ports::runtime::truthy(&Value::Null){Value::Null}else{Value::Null})]}
}
