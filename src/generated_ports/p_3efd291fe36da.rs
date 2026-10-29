//! Native Rust UI structure derived from original: src/components/LogoV2/AnimatedClawd.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "Box".into(),
        attrs: vec![
            ("height".into(), Value::Null),
            ("flexDirection".into(), Value::String("column".into())),
            ("onClick".into(), Value::Null),
        ]
        .into_iter()
        .collect(),
        children: vec![VNode::Value(Value::Null)],
    }
}
