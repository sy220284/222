//! Native Rust UI structure derived from original: src/commands/install-github-app/install-github-app.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"WorkflowMultiselectDialog".into(),attrs:vec![("defaultSelections".into(),crate::generated_ports::runtime::property(&Value::Null,"selectedWorkflows")),("onSubmit".into(),Value::Null)].into_iter().collect(),children:vec![]}
}
