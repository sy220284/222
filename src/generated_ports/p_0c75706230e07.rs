//! Native Rust UI structure derived from original: src/components/TrustDialog/TrustDialog.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element{tag:"Text".into(),attrs:vec![].into_iter().collect(),children:vec![VNode::Text("Quick safety check: Is this a project you created or one you trust? (Like your own code, a well-known open source project, or work from your team). If not, take a moment to review what".into()),VNode::Value(Value::String("'".into())),VNode::Text("s in this folder first.".into())]}
}
