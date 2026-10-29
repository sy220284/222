//! Native Rust UI structure derived from original: src/components/DevChannelsDialog.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element{tag:"Text".into(),attrs:vec![].into_iter().collect(),children:vec![VNode::Text("--dangerously-load-development-channels is for local channel development only. Do not use this option to run channels you have downloaded off the internet.".into())]}
}
