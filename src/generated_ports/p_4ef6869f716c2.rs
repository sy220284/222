//! Native Rust UI structure derived from original: src/components/CompactSummary.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "MessageResponse".into(),
        attrs: vec![].into_iter().collect(),
        children: vec![VNode::Element {
            tag: "Box".into(),
            attrs: vec![("flexDirection".into(), Value::String("column".into()))]
                .into_iter()
                .collect(),
            children: vec![
                VNode::Element {
                    tag: "Text".into(),
                    attrs: vec![("dimColor".into(), Value::Bool(true))]
                        .into_iter()
                        .collect(),
                    children: vec![
                        VNode::Text("Summarized ".into()),
                        VNode::Value(crate::generated_ports::runtime::property(
                            &Value::Null,
                            "messagesSummarized",
                        )),
                        VNode::Text("messages".into()),
                        VNode::Value(Value::String(" ".into())),
                        VNode::Value(if crate::generated_ports::runtime::truthy(&Value::Null) {
                            Value::String("up to this point".into())
                        } else {
                            Value::String("from this point".into())
                        }),
                    ],
                },
                VNode::Value(Value::Null),
                VNode::Element {
                    tag: "Text".into(),
                    attrs: vec![("dimColor".into(), Value::Bool(true))]
                        .into_iter()
                        .collect(),
                    children: vec![VNode::Element {
                        tag: "ConfigurableShortcutHint".into(),
                        attrs: vec![
                            (
                                "action".into(),
                                Value::String("app:toggleTranscript".into()),
                            ),
                            ("context".into(), Value::String("Global".into())),
                            ("fallback".into(), Value::String("ctrl+o".into())),
                            ("description".into(), Value::String("expand history".into())),
                            ("parens".into(), Value::Bool(true)),
                        ]
                        .into_iter()
                        .collect(),
                        children: vec![],
                    }],
                },
            ],
        }],
    }
}
