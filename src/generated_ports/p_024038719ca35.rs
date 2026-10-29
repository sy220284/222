//! Native Rust UI structure derived from original: src/components/Spinner/TeammateSpinnerTree.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "TeammateSpinnerLine".into(),
        attrs: vec![
            (
                "key".into(),
                crate::generated_ports::runtime::property(&Value::Null, "id"),
            ),
            ("teammate".into(), Value::Null),
            ("isLast".into(), Value::Null),
            ("isSelected".into(), Value::Null),
            ("isForegrounded".into(), Value::Null),
            ("allIdle".into(), Value::Null),
            ("showPreview".into(), Value::Null),
        ]
        .into_iter()
        .collect(),
        children: vec![],
    }
}
