//! Native Rust UI structure derived from original: src/components/TeleportStash.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"Dialog".into(),attrs:vec![("title".into(),Value::String("Working Directory Has Changes".into())),("onCancel".into(),Value::Null)].into_iter().collect(),children:vec![VNode::Empty,VNode::Element{tag:"Text".into(),attrs:vec![].into_iter().collect(),children:vec![VNode::Text("Teleport will switch git branches. The following changes were found:\n      ".into())]},VNode::Empty,VNode::Element{tag:"Box".into(),attrs:vec![("flexDirection".into(),Value::String("column".into())),("paddingLeft".into(),serde_json::json!(2))].into_iter().collect(),children:vec![VNode::Empty,VNode::Value(if crate::generated_ports::runtime::truthy(&Value::Null){if crate::generated_ports::runtime::truthy(&Value::Null){Value::Null}else{Value::Null}}else{Value::Null}),VNode::Empty]},VNode::Empty,VNode::Element{tag:"Text".into(),attrs:vec![].into_iter().collect(),children:vec![VNode::Text("Would you like to stash these changes and continue with teleport?\n      ".into())]},VNode::Empty,VNode::Value(if crate::generated_ports::runtime::truthy(&Value::Null){Value::Null}else{Value::Null}),VNode::Empty]}
}
