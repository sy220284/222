//! Native Rust UI structure derived from original: src/components/PromptInput/PromptInputHelpMenu.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Box".into(),
        attrs: vec![].into_iter().collect(),
        children: vec![VNode::Element {
            tag: "Text".into(),
            attrs: vec![("dimColor".into(), Value::Null)].into_iter().collect(),
            children: vec![
                VNode::Value(Value::Null),
                VNode::Value(Value::String(" ".into())),
                VNode::Value(
                    if crate::generated_ports::runtime::truthy(&Value::Bool(false)) {
                        Value::String("to cycle modes".into())
                    } else {
                        Value::String("to auto-accept edits".into())
                    },
                ),
            ],
        }],
    }
}
