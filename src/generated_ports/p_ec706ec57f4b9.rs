//! Native Rust port derived from original: src/components/Spinner/ShimmerChar.tsx
//!
//! Shimmer highlight decision: the glimmer index itself and its single
//! neighbor take the shimmer color; all other indices keep the message
//! color. Text view carries the resolved color key.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use crate::generated_ports::ui::VNode;

/// `ShimmerChar` color decision.
pub fn shimmer_color<'a>(
    index: usize,
    glimmer_index: usize,
    message_color: &'a str,
    shimmer_color_key: &'a str,
) -> &'a str {
    if index == glimmer_index || index.abs_diff(glimmer_index) == 1 {
        shimmer_color_key
    } else {
        message_color
    }
}

pub fn render_shimmer_char(char_value: &str, index: usize, glimmer_index: usize) -> VNode {
    let color = shimmer_color(index, glimmer_index, "messageColor", "shimmerColor");
    VNode::Element {
        tag: "Text".into(),
        attrs: [("color".into(), color.into())].into_iter().collect(),
        children: vec![VNode::Text(char_value.into())],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glimmer_and_neighbor_highlighted() {
        assert_eq!(shimmer_color(3, 3, "msg", "shim"), "shim");
        assert_eq!(shimmer_color(2, 3, "msg", "shim"), "shim");
        assert_eq!(shimmer_color(4, 3, "msg", "shim"), "shim");
        assert_eq!(shimmer_color(1, 3, "msg", "shim"), "msg");
    }
}
