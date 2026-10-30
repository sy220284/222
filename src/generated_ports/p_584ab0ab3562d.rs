//! Native Rust UI structure derived from original: src/components/agents/AgentsMenu.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "AgentsList".into(),
        attrs: vec![
            (
                "source".into(),
                crate::generated_ports::runtime::property(&Value::Null, "source"),
            ),
            ("agents".into(), Value::Null),
            ("onBack".into(), Value::Null),
            ("onSelect".into(), Value::Null),
            ("onCreateNew".into(), Value::Null),
            ("changes".into(), Value::Null),
        ]
        .into_iter()
        .collect(),
        children: vec![],
    }
}
