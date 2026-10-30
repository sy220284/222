//! Hand-port of src/components/design-system/Divider.tsx.
#![allow(dead_code)]
use crate::{generated_ports::ui::VNode, portable_ink_core::LineWidthCache};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub fn render_source_view(props: &Value) -> VNode {
    // The source defaults to useTerminalSize().columns. Source inspection is
    // context-free, so its terminal hook value is explicitly provided.
    let columns = props
        .get("width")
        .and_then(Value::as_u64)
        .or_else(|| props.get("terminalColumns").and_then(Value::as_u64))
        .expect("width or terminalColumns required to evaluate Divider") as usize;
    let padding = props.get("padding").and_then(Value::as_u64).unwrap_or(0) as usize;
    let effective_width = columns.saturating_sub(padding);
    let ch = props.get("char").and_then(Value::as_str).unwrap_or("─");
    let color = props.get("color");
    let title = props
        .get("title")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty());
    let mut attrs = BTreeMap::from([("dimColor".into(), json!(color.is_none()))]);
    if let Some(color) = color {
        attrs.insert("color".into(), color.clone());
    }

    let children = if let Some(title) = title {
        let title_width = LineWidthCache::default()
            .line_width(title)
            .saturating_add(2);
        let side_width = effective_width.saturating_sub(title_width);
        let left_width = side_width / 2;
        let right_width = side_width - left_width;
        vec![
            VNode::Text(ch.repeat(left_width)),
            VNode::Text(" ".into()),
            VNode::Element {
                tag: "Text".into(),
                attrs: BTreeMap::from([("dimColor".into(), json!(true))]),
                children: vec![VNode::Element {
                    tag: "Ansi".into(),
                    attrs: BTreeMap::new(),
                    children: vec![VNode::Text(title.into())],
                }],
            },
            VNode::Text(" ".into()),
            VNode::Text(ch.repeat(right_width)),
        ]
    } else {
        vec![VNode::Text(ch.repeat(effective_width))]
    };
    VNode::Element {
        tag: "Text".into(),
        attrs,
        children,
    }
}

#[cfg(test)]
mod parity_tests {
    use super::*;
    #[test]
    fn plain_divider_respects_terminal_width_padding_and_color() {
        let node = render_source_view(&json!({"terminalColumns":10,"padding":2}));
        assert_eq!(node.text_content(), "────────");
        assert_eq!(node.unresolved_node_count(), 0);
        let colored = render_source_view(&json!({"width":3,"char":"=","color":"muted"}));
        assert_eq!(colored.text_content(), "===");
        let VNode::Element { attrs, .. } = colored else {
            panic!("Text")
        };
        assert_eq!(attrs.get("color"), Some(&json!("muted")));
        assert_eq!(attrs.get("dimColor"), Some(&json!(false)));
    }
    #[test]
    fn centered_title_matches_original_even_when_width_is_short() {
        let node = render_source_view(&json!({"width":11,"title":"Hi"}));
        assert_eq!(node.text_content(), "─── Hi ────");
        let short = render_source_view(&json!({"width":2,"title":"Title"}));
        assert_eq!(short.text_content(), " Title ");
    }
}
