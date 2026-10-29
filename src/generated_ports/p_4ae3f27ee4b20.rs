//! Native Rust UI structure derived from original: src/components/PromptInput/PromptInputModeIndicator.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"Box".into(),attrs:vec![("alignItems".into(),Value::String("flex-start".into())),("alignSelf".into(),Value::String("flex-start".into())),("flexWrap".into(),Value::String("nowrap".into())),("justifyContent".into(),Value::String("flex-start".into()))].into_iter().collect(),children:vec![VNode::Value(if crate::generated_ports::runtime::truthy(&Value::Null){Value::Null}else{if crate::generated_ports::runtime::truthy(&Value::Null){Value::Null}else{Value::Null}})]}
}
