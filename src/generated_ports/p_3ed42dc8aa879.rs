//! Manual view port of src/components/PromptInput/HistorySearchInput.tsx.
#![allow(dead_code)]
use crate::{generated_ports::ui::VNode, portable_ink_core::LineWidthCache};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub fn render_source_view(props: &Value) -> VNode {
    let value = props
        .get("value")
        .and_then(Value::as_str)
        .expect("HistorySearchInput requires value");
    let failed = props.get("historyFailedMatch").and_then(Value::as_bool) == Some(true);
    let hint = if failed {
        "no matching prompt:"
    } else {
        "search prompts:"
    };
    let width = LineWidthCache::default().line_width(value) + 1;
    let cursor_offset = value.encode_utf16().count();
    VNode::Element {
        tag: "Box".into(),
        attrs: BTreeMap::from([("gap".into(), json!(1))]),
        children: vec![
            VNode::Element {
                tag: "Text".into(),
                attrs: BTreeMap::from([("dimColor".into(), json!(true))]),
                children: vec![VNode::Text(hint.into())],
            },
            VNode::Element {
                tag: "TextInput".into(),
                attrs: BTreeMap::from([
                    ("value".into(), json!(value)),
                    ("cursorOffset".into(), json!(cursor_offset)),
                    ("columns".into(), json!(width)),
                    ("focus".into(), json!(true)),
                    ("showCursor".into(), json!(true)),
                    ("multiline".into(), json!(false)),
                    ("dimColor".into(), json!(true)),
                ]),
                children: vec![],
            },
        ],
    }
}

#[cfg(test)]
mod source_parity_tests {
    use super::*;
    #[test]
    fn history_label_and_input_cursor_track_content() {
        let view = render_source_view(&json!({"value":"hello", "historyFailedMatch":false}));
        assert!(view.text_content().contains("search prompts:"));
        assert_eq!(view.unresolved_node_count(), 0);
        let VNode::Element { children, .. } = view else {
            panic!("Box")
        };
        let VNode::Element { attrs, .. } = &children[1] else {
            panic!("TextInput")
        };
        assert_eq!(attrs.get("columns"), Some(&json!(6)));
        assert_eq!(attrs.get("cursorOffset"), Some(&json!(5)));
        assert!(
            render_source_view(&json!({"value":"", "historyFailedMatch":true}))
                .text_content()
                .contains("no matching prompt:")
        );
    }
}
