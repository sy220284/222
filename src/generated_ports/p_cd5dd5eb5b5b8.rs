//! Native Rust UI structure derived from original: src/components/Settings/Settings.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"Tab".into(),attrs:vec![("key".into(),Value::String("config".into())),("title".into(),Value::String("Config".into()))].into_iter().collect(),children:vec![VNode::Element{tag:"Suspense".into(),attrs:vec![("fallback".into(),Value::Null)].into_iter().collect(),children:vec![VNode::Element{tag:"Config".into(),attrs:vec![("context".into(),Value::Null),("onClose".into(),Value::Null),("setTabsHidden".into(),Value::Null),("onIsSearchModeChange".into(),Value::Null),("contentHeight".into(),Value::Null)].into_iter().collect(),children:vec![]}]}]}
}
