//! Native Rust UI structure derived from original: src/components/SearchBox.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Group(vec![VNode::Value(
        if crate::generated_ports::runtime::truthy(&Value::Null) {
            if crate::generated_ports::runtime::truthy(&Value::Null) {
                Value::Null
            } else {
                Value::Null
            }
        } else {
            if crate::generated_ports::runtime::truthy(&Value::Null) {
                Value::Null
            } else {
                Value::Null
            }
        },
    )])
}
