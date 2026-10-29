//! Native Rust UI structure derived from original: src/components/agents/new-agent-creation/CreateAgentWizard.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "WizardProvider".into(),
        attrs: vec![
            ("steps".into(), Value::Null),
            ("initialData".into(), Value::Null),
            ("onComplete".into(), Value::Null),
            ("onCancel".into(), Value::Null),
            ("title".into(), Value::String("Create new agent".into())),
            ("showStepCounter".into(), Value::Bool(false)),
        ]
        .into_iter()
        .collect(),
        children: vec![],
    }
}
