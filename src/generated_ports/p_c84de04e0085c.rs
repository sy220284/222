//! Rust exports derived from original: src/hooks/useReplBridge.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;
pub fn BRIDGE_FAILURE_DISMISS_MS() -> Value {
    serde_json::from_str(r#"10000"#).expect("Valid source constant")
}

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Group(vec![
        VNode::Empty,
        VNode::Element {
            tag: "Text".into(),
            attrs: vec![("color".into(), Value::String("error".into()))]
                .into_iter()
                .collect(),
            children: vec![VNode::Text("Remote Control failed".into())],
        },
        VNode::Empty,
        VNode::Value(Value::Null),
        VNode::Empty,
    ])
}
