//! Native Rust UI structure derived from original: src/components/NotebookEditToolUseRejectedMessage.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"Box".into(),attrs:vec![("marginTop".into(),serde_json::json!(1)),("flexDirection".into(),Value::String("column".into()))].into_iter().collect(),children:vec![VNode::Element{tag:"HighlightedCode".into(),attrs:vec![("code".into(),Value::Null),("filePath".into(),if crate::generated_ports::runtime::truthy(&Value::Null){Value::String("file.md".into())}else{Value::String("file.py".into())}),("dim".into(),Value::Bool(true))].into_iter().collect(),children:vec![]}]}
}
