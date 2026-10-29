//! Native Rust UI structure derived from original: src/components/agents/new-agent-creation/wizard-steps/ConfirmStepWrapper.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"ConfirmStep".into(),attrs:vec![("tools".into(),Value::Null),("existingAgents".into(),Value::Null),("onSave".into(),Value::Null),("onSaveAndEdit".into(),Value::Null),("error".into(),Value::Null)].into_iter().collect(),children:vec![]}
}
