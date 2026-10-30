//! Hand-port of src/components/design-system/ProgressBar.tsx.
#![allow(dead_code)]
use serde_json::{json, Value};
use std::collections::BTreeMap;
use crate::generated_ports::ui::VNode;
const BLOCKS: [&str; 9] = [" ", "▏", "▎", "▍", "▌", "▋", "▊", "▉", "█"];

pub fn render_source_view(props: &Value) -> VNode {
    let ratio = props.get("ratio").and_then(Value::as_f64).expect("ratio is a required number").clamp(0.0, 1.0);
    let width = props.get("width").and_then(Value::as_u64).expect("width is a required nonnegative integer") as usize;
    let whole = (ratio * width as f64).floor() as usize;
    let mut bar = BLOCKS[8].repeat(whole);
    if whole < width {
        let remainder = ratio * width as f64 - whole as f64;
        let partial = ((remainder * BLOCKS.len() as f64).floor() as usize).min(8);
        bar.push_str(BLOCKS[partial]);
        bar.push_str(&BLOCKS[0].repeat(width - whole - 1));
    }
    let mut attrs = BTreeMap::new();
    if let Some(color) = props.get("fillColor") { attrs.insert("color".into(), color.clone()); }
    if let Some(color) = props.get("emptyColor") { attrs.insert("backgroundColor".into(), color.clone()); }
    VNode::Element { tag: "Text".into(), attrs, children: vec![VNode::Text(bar)] }
}

#[cfg(test)]
mod source_parity_tests {
    use super::*;
    #[test]
    fn fractional_progress_uses_nine_source_glyphs() {
        for (ratio, width, expected) in [(0.0, 4, "    "), (1.0, 4, "████"), (0.5, 5, "██▌  "), (0.25, 4, "█   "), (2.0, 3, "███"), (-1.0, 3, "   ")] {
            let node = render_source_view(&json!({"ratio":ratio,"width":width}));
            assert_eq!(node.text_content(),expected);
            assert_eq!(node.unresolved_node_count(),0);
        }
    }
    #[test]
    fn preserves_source_color_attributes() {
        let node = render_source_view(&json!({"ratio":0.5,"width":2,"fillColor":"success","emptyColor":"muted"}));
        let VNode::Element { attrs, .. } = node else {panic!("text")};
        assert_eq!(attrs.get("color"),Some(&json!("success")));
        assert_eq!(attrs.get("backgroundColor"),Some(&json!("muted")));
    }
}
