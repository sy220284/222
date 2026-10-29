//! Native Rust UI structure derived from original: src/components/ApproveApiKey.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"Select".into(),attrs:vec![("defaultValue".into(),Value::String("no".into())),("defaultFocusValue".into(),Value::String("no".into())),("options".into(),Value::Null),("onChange".into(),Value::Null),("onCancel".into(),Value::Null)].into_iter().collect(),children:vec![]}
}
