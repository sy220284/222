//! Native Rust UI structure derived from original: src/components/AutoUpdaterWrapper.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "PackageManagerAutoUpdater".into(),
        attrs: vec![
            ("verbose".into(), Value::Null),
            ("onAutoUpdaterResult".into(), Value::Null),
            ("autoUpdaterResult".into(), Value::Null),
            ("isUpdating".into(), Value::Null),
            ("onChangeIsUpdating".into(), Value::Null),
            ("showSuccessMessage".into(), Value::Null),
        ]
        .into_iter()
        .collect(),
        children: vec![],
    }
}
