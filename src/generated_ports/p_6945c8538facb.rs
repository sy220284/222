//! Native Rust UI structure derived from original: src/components/RemoteEnvironmentDialog.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "MultipleEnvironmentsContent".into(),
        attrs: vec![
            ("environments".into(), Value::Null),
            ("selectedEnvironment".into(), Value::Null),
            ("selectedEnvironmentSource".into(), Value::Null),
            ("loadingState".into(), Value::Null),
            ("onSelect".into(), Value::Null),
            ("onCancel".into(), Value::Null),
        ]
        .into_iter()
        .collect(),
        children: vec![],
    }
}
