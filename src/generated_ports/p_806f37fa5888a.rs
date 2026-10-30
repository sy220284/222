//! Native Rust port derived from original: src/components/PressEnterToContinue.tsx
//!
//! "Press Enter to continue…" hint view (permission-colored text with
//! bold Enter). Verbatim string and color key.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use crate::generated_ports::ui::VNode;

pub const PRESS_ENTER_TEXT: &str = "Press Enter to continue\u{2026}";

pub fn press_enter_to_continue() -> VNode {
    VNode::Element {
        tag: "Text".into(),
        attrs: [("color".into(), "permission".into())]
            .into_iter()
            .collect(),
        children: vec![VNode::Text(PRESS_ENTER_TEXT.into())],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_and_color_key_verbatim() {
        match press_enter_to_continue() {
            VNode::Element {
                attrs, children, ..
            } => {
                assert!(attrs.iter().any(|(k, v)| k == "color" && v == "permission"));
                assert_eq!(children.len(), 1);
            }
            _ => panic!(),
        }
        assert_eq!(PRESS_ENTER_TEXT, "Press Enter to continue\u{2026}");
    }
}
