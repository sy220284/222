//! Rust exports derived from original: src/components/PromptInput/Notifications.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;
pub fn FOOTER_TEMPORARY_STATUS_TIMEOUT() -> Value {
    serde_json::from_str(r#"5000"#).expect("Valid source constant")
}

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Group(vec![
        VNode::Empty,
        VNode::Element {
            tag: "IdeStatusIndicator".into(),
            attrs: vec![
                ("ideSelection".into(), Value::Null),
                ("mcpClients".into(), Value::Null),
            ]
            .into_iter()
            .collect(),
            children: vec![],
        },
        VNode::Empty,
        VNode::Value(Value::Null),
        VNode::Empty,
        VNode::Value(Value::Null),
        VNode::Empty,
        VNode::Value(Value::Null),
        VNode::Empty,
        VNode::Value(Value::Null),
        VNode::Empty,
        VNode::Value(Value::Null),
        VNode::Empty,
        VNode::Value(Value::Null),
        VNode::Empty,
        VNode::Value(Value::Null),
        VNode::Empty,
        VNode::Value(Value::Null),
        VNode::Empty,
        VNode::Value(if crate::generated_ports::runtime::truthy(&Value::Null) {
            Value::Null
        } else {
            Value::Null
        }),
        VNode::Empty,
        VNode::Element {
            tag: "MemoryUsageIndicator".into(),
            attrs: vec![].into_iter().collect(),
            children: vec![],
        },
        VNode::Empty,
        VNode::Element {
            tag: "SandboxPromptFooterHint".into(),
            attrs: vec![].into_iter().collect(),
            children: vec![],
        },
        VNode::Empty,
    ])
}
