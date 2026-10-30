//! Rust exports derived from original: src/components/messages/teamMemCollapsed.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::{json, Value};

pub fn checkHasTeamMemOps(args: &[Value]) -> Value {
    super::runtime::or(
        &super::runtime::or(
            &super::runtime::gt(
                &super::runtime::nullish(
                    &super::runtime::property(
                        &args.first().cloned().unwrap_or(Value::Null),
                        "teamMemorySearchCount",
                    ),
                    &json!(0),
                ),
                &json!(0),
            ),
            &super::runtime::gt(
                &super::runtime::nullish(
                    &super::runtime::property(
                        &args.first().cloned().unwrap_or(Value::Null),
                        "teamMemoryReadCount",
                    ),
                    &json!(0),
                ),
                &json!(0),
            ),
        ),
        &super::runtime::gt(
            &super::runtime::nullish(
                &super::runtime::property(
                    &args.first().cloned().unwrap_or(Value::Null),
                    "teamMemoryWriteCount",
                ),
                &json!(0),
            ),
            &json!(0),
        ),
    )
}

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Text".into(),
        attrs: vec![("key".into(), Value::String("team-mem-write".into()))]
            .into_iter()
            .collect(),
        children: vec![
            VNode::Value(Value::Null),
            VNode::Empty,
            VNode::Value(Value::Null),
            VNode::Text("team".into()),
            VNode::Value(Value::String(" ".into())),
            VNode::Value(Value::Null),
        ],
    }
}
