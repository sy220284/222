//! Rust exports derived from original: src/components/permissions/FilePermissionDialog/permissionOptions.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FileOperationType {
    #[serde(rename = "read")]
    read,
    #[serde(rename = "write")]
    write,
    #[serde(rename = "create")]
    create,
}

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Text".into(),
        attrs: vec![].into_iter().collect(),
        children: vec![
            VNode::Text("Yes, allow all edits in ".into()),
            VNode::Element {
                tag: "Text".into(),
                attrs: vec![("bold".into(), Value::Bool(true))]
                    .into_iter()
                    .collect(),
                children: vec![VNode::Value(Value::Null), VNode::Text("/".into())],
            },
            VNode::Text("during this\n            session ".into()),
            VNode::Element {
                tag: "Text".into(),
                attrs: vec![("bold".into(), Value::Bool(true))]
                    .into_iter()
                    .collect(),
                children: vec![
                    VNode::Text("(".into()),
                    VNode::Value(Value::Null),
                    VNode::Text(")".into()),
                ],
            },
            VNode::Empty,
        ],
    }
}
