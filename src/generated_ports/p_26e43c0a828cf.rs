//! Native Rust UI structure derived from original: src/components/ExportDialog.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"Dialog".into(),attrs:vec![("title".into(),Value::String("Export Conversation".into())),("subtitle".into(),Value::String("Select export method:".into())),("color".into(),Value::String("permission".into())),("onCancel".into(),Value::Null),("inputGuide".into(),Value::Null),("isCancelActive".into(),Value::Bool(!crate::generated_ports::runtime::truthy(&Value::Null)))].into_iter().collect(),children:vec![VNode::Empty,VNode::Value(if crate::generated_ports::runtime::truthy(&Value::Bool(!crate::generated_ports::runtime::truthy(&Value::Null))){Value::Null}else{Value::Null}),VNode::Empty]}
}
