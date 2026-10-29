//! Native Rust UI structure derived from original: src/commands/plugin/PluginOptionsFlow.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "PluginOptionsDialog".into(),
        attrs: vec![
            (
                "key".into(),
                crate::generated_ports::runtime::property(&Value::Null, "key"),
            ),
            (
                "title".into(),
                crate::generated_ports::runtime::property(&Value::Null, "title"),
            ),
            (
                "subtitle".into(),
                crate::generated_ports::runtime::property(&Value::Null, "subtitle"),
            ),
            (
                "configSchema".into(),
                crate::generated_ports::runtime::property(&Value::Null, "schema"),
            ),
            ("initialValues".into(), Value::Null),
            ("onSave".into(), Value::Null),
            ("onCancel".into(), Value::Null),
        ]
        .into_iter()
        .collect(),
        children: vec![],
    }
}
