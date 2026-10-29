//! Native Rust UI structure derived from original: src/components/CustomSelect/select-option.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"ListItem".into(),attrs:vec![("isFocused".into(),Value::Null),("isSelected".into(),Value::Null),("description".into(),Value::Null),("showScrollDown".into(),Value::Null),("showScrollUp".into(),Value::Null),("styled".into(),Value::Bool(false)),("declareCursor".into(),Value::Null)].into_iter().collect(),children:vec![VNode::Value(Value::Null)]}
}
