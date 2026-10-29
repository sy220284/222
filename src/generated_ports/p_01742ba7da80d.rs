//! Native Rust UI structure derived from original: src/components/IdeAutoConnectDialog.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Dialog".into(),
        attrs: vec![
            (
                "title".into(),
                Value::String("Do you wish to disable auto-connect to IDE?".into()),
            ),
            (
                "subtitle".into(),
                Value::String("You can also configure this in /config".into()),
            ),
            ("onCancel".into(), Value::Null),
            ("color".into(), Value::String("ide".into())),
        ]
        .into_iter()
        .collect(),
        children: vec![VNode::Value(Value::Null)],
    }
}
