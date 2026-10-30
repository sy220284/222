//! Hand-port of src/components/SearchBox.tsx. No opaque JSX expressions remain.
#![allow(dead_code)]
use crate::generated_ports::ui::VNode;
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn text(children: Vec<VNode>, attrs: &[(&str, Value)]) -> VNode {
    VNode::Element {
        tag: "Text".into(),
        attrs: attrs
            .iter()
            .map(|(k, v)| ((*k).into(), v.clone()))
            .collect(),
        children,
    }
}
fn utf16_slice(s: &str, start: usize, end: usize) -> String {
    String::from_utf16_lossy(
        &s.encode_utf16()
            .skip(start)
            .take(end.saturating_sub(start))
            .collect::<Vec<_>>(),
    )
}

pub fn render_source_view(props: &Value) -> VNode {
    let query = props.get("query").and_then(Value::as_str).unwrap_or("");
    let placeholder = props
        .get("placeholder")
        .and_then(Value::as_str)
        .unwrap_or("Search…");
    let prefix = props.get("prefix").and_then(Value::as_str).unwrap_or("⌕");
    let focused = props.get("isFocused").and_then(Value::as_bool) == Some(true);
    let terminal_focused = props.get("isTerminalFocused").and_then(Value::as_bool) == Some(true);
    let borderless = props.get("borderless").and_then(Value::as_bool) == Some(true);
    let utf16_length = query.encode_utf16().count();
    let offset = props
        .get("cursorOffset")
        .and_then(Value::as_u64)
        .map_or(utf16_length, |value| {
            usize::try_from(value).unwrap_or(usize::MAX)
        });

    let content = if !query.is_empty() {
        if focused && terminal_focused {
            let mut nodes = vec![text(vec![VNode::Text(utf16_slice(query, 0, offset))], &[])];
            let cursor = if offset < utf16_length {
                utf16_slice(query, offset, offset + 1)
            } else {
                " ".into()
            };
            nodes.push(text(vec![VNode::Text(cursor)], &[("inverse", json!(true))]));
            if offset < utf16_length {
                nodes.push(text(
                    vec![VNode::Text(utf16_slice(query, offset + 1, utf16_length))],
                    &[],
                ));
            }
            nodes
        } else {
            vec![text(vec![VNode::Text(query.into())], &[])]
        }
    } else if focused && terminal_focused {
        let units = placeholder.encode_utf16().count();
        vec![
            text(
                vec![VNode::Text(utf16_slice(placeholder, 0, 1))],
                &[("inverse", json!(true))],
            ),
            text(
                vec![VNode::Text(utf16_slice(placeholder, 1, units))],
                &[("dimColor", json!(true))],
            ),
        ]
    } else {
        vec![text(
            vec![VNode::Text(placeholder.into())],
            &[("dimColor", json!(focused))],
        )]
    };
    let mut prefix_and_query = vec![VNode::Text(prefix.into()), VNode::Text(" ".into())];
    prefix_and_query.extend(content);
    let contents = text(prefix_and_query, &[("dimColor", json!(!focused))]);
    let mut attrs = BTreeMap::from([
        ("flexShrink".into(), json!(0)),
        ("borderDimColor".into(), json!(!focused)),
        ("paddingX".into(), json!(if borderless { 0 } else { 1 })),
    ]);
    if !borderless {
        attrs.insert("borderStyle".into(), json!("round"));
    }
    if focused {
        attrs.insert("borderColor".into(), json!("suggestion"));
    }
    if let Some(width) = props.get("width") {
        attrs.insert("width".into(), width.clone());
    }
    VNode::Element {
        tag: "Box".into(),
        attrs,
        children: vec![contents],
    }
}

#[cfg(test)]
mod parity_tests {
    use super::*;
    #[test]
    fn search_box_focused_and_terminal_cursor_matches_source() {
        let view = render_source_view(
            &json!({"query":"abc", "isFocused":true, "isTerminalFocused":true, "cursorOffset":1, "width":30}),
        );
        assert_eq!(view.text_content(), "⌕ abc");
        assert_eq!(view.unresolved_node_count(), 0);
        let VNode::Element {
            tag,
            attrs,
            children,
        } = view
        else {
            panic!("expected Box")
        };
        assert_eq!(tag, "Box");
        assert_eq!(attrs.get("borderStyle"), Some(&json!("round")));
        assert_eq!(attrs.get("borderColor"), Some(&json!("suggestion")));
        assert_eq!(attrs.get("width"), Some(&json!(30)));
        let VNode::Element { children, .. } = &children[0] else {
            panic!("Text")
        };
        let VNode::Element { attrs, .. } = &children[3] else {
            panic!("cursor")
        };
        assert_eq!(attrs.get("inverse"), Some(&json!(true)));
    }
    #[test]
    fn search_box_placeholder_and_focus_states() {
        let view = render_source_view(
            &json!({"query":"", "isFocused":true, "isTerminalFocused":true,"borderless":true}),
        );
        assert_eq!(view.text_content(), "⌕ Search…");
        let VNode::Element { attrs, .. } = view else {
            panic!("box")
        };
        assert!(!attrs.contains_key("borderStyle"));
        assert_eq!(attrs.get("paddingX"), Some(&json!(0)));
        let plain = render_source_view(
            &json!({"query":"hello", "isFocused":false, "isTerminalFocused":false}),
        );
        assert_eq!(plain.text_content(), "⌕ hello");
        assert_eq!(plain.unresolved_node_count(), 0);
    }
}
