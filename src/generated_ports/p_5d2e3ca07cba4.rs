//! Native Rust UI structure derived from original: src/commands/privacy-settings/privacy-settings.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "PrivacySettingsDialog".into(),
        attrs: vec![
            ("settings".into(), Value::Null),
            (
                "domainExcluded".into(),
                crate::generated_ports::runtime::property(&Value::Null, "domain_excluded"),
            ),
            ("onDone".into(), Value::Null),
        ]
        .into_iter()
        .collect(),
        children: vec![],
    }
}
