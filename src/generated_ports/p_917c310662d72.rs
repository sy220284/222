//! Native Rust UI structure derived from original: src/components/CustomSelect/select-input-option.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Group(vec![VNode::Element{tag:"Text".into(),attrs:vec![("color".into(),if crate::generated_ports::runtime::truthy(&Value::Null){Value::String("suggestion".into())}else{Value::Null})].into_iter().collect(),children:vec![VNode::Value(crate::generated_ports::runtime::property(&Value::Null,"label"))]},VNode::Value(if crate::generated_ports::runtime::truthy(&Value::Null){Value::Null}else{Value::Null})])
}
