//! Hand port of `src/components/BaseTextInput.tsx` rendering branches.
//! The host terminal still owns key input, paste callbacks and cursor events;
//! this view consumes the same input-state snapshot as the original component.
#![allow(dead_code)]
use crate::generated_ports::{
    p_2f84f9e35605c,
    text_input_semantics::{render_placeholder, viewport_highlights},
    ui::VNode,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn element(tag: &str, attrs: BTreeMap<String, Value>, children: Vec<VNode>) -> VNode {
    VNode::Element {
        tag: tag.into(),
        attrs,
        children,
    }
}
fn child(value: &Value) -> Option<VNode> {
    if value.is_null() {
        None
    } else {
        Some(
            serde_json::from_value::<VNode>(value.clone()).unwrap_or_else(|_| match value {
                Value::String(s) => VNode::Text(s.clone()),
                _ => VNode::Value(value.clone()),
            }),
        )
    }
}
fn argument_hint(props: &Value) -> Option<VNode> {
    let value = props.get("value").and_then(Value::as_str).unwrap_or("");
    let hint = props
        .get("argumentHint")
        .and_then(Value::as_str)
        .unwrap_or("");
    // The source shows a slash-command hint after an unparameterized command,
    // or after any command whose current input ends with a space.
    if hint.is_empty()
        || value.is_empty()
        || !value.starts_with('/')
        || !(value.trim().find(' ').is_none() || value.ends_with(' '))
    {
        return None;
    }
    let prefix = if value.ends_with(' ') { "" } else { " " };
    Some(element(
        "Text",
        BTreeMap::from([("dimColor".into(), json!(true))]),
        vec![VNode::Text(format!("{prefix}{hint}"))],
    ))
}

pub fn render_source_view(props: &Value) -> VNode {
    let state = props
        .get("inputState")
        .expect("BaseTextInput requires inputState");
    let text = state
        .get("renderedValue")
        .and_then(Value::as_str)
        .expect("BaseTextInput requires inputState.renderedValue");
    let focus = props.get("focus").and_then(Value::as_bool).unwrap_or(false);
    let show_cursor = props
        .get("showCursor")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let terminal_focus = props
        .get("terminalFocus")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let cursor = json!({
        "line": state.get("cursorLine").and_then(Value::as_u64).unwrap_or(0),
        "column": state.get("cursorColumn").and_then(Value::as_u64).unwrap_or(0),
        "active": focus && show_cursor && terminal_focus,
    });
    let attrs = BTreeMap::from([("cursor".into(), cursor)]);
    let highlights = viewport_highlights(props);
    let hint = argument_hint(props);
    let children = props.get("children").and_then(child);
    if !highlights.is_empty() {
        let highlighted = p_2f84f9e35605c::render_source_view(&json!({
            "text": text,"highlights": highlights,"time":props.get("time").and_then(Value::as_i64).unwrap_or(0),
        }));
        let mut kids = vec![highlighted];
        if let Some(hint) = hint {
            kids.push(hint);
        }
        if let Some(children) = children {
            kids.push(children);
        }
        return element("Box", attrs, kids);
    }
    let placeholder = render_placeholder(&json!({
        "placeholder":props.get("placeholder"),
        "value":props.get("value"),
        "showCursor":props.get("showCursor"),
        "focus":props.get("focus"),
        "terminalFocus":terminal_focus,
        "hidePlaceholderText":props.get("hidePlaceholderText"),
    }));
    let display = if placeholder["showPlaceholder"] == true {
        props
            .get("placeholderElement")
            .and_then(child)
            .unwrap_or_else(|| {
                element(
                    "Ansi",
                    BTreeMap::new(),
                    vec![VNode::Text(
                        placeholder["renderedPlaceholder"]
                            .as_str()
                            .unwrap_or("")
                            .into(),
                    )],
                )
            })
    } else {
        element("Ansi", BTreeMap::new(), vec![VNode::Text(text.into())])
    };
    let mut kids = vec![display];
    if let Some(hint) = hint {
        kids.push(hint);
    }
    if let Some(children) = children {
        kids.push(children);
    }
    let mut text_attrs = BTreeMap::from([("wrap".into(), json!("truncate-end"))]);
    if let Some(dim) = props.get("dimColor") {
        text_attrs.insert("dimColor".into(), dim.clone());
    }
    element("Box", attrs, vec![element("Text", text_attrs, kids)])
}

#[cfg(test)]
mod parity_tests {
    use super::*;
    #[test]
    fn focused_placeholder_and_cursor_are_rendered() {
        let view = render_source_view(
            &json!({"inputState":{"renderedValue":"","cursorLine":2,"cursorColumn":4},
            "placeholder":"Hello","value":"","showCursor":true,"focus":true,"terminalFocus":true}),
        );
        let VNode::Element {
            attrs, children, ..
        } = view
        else {
            panic!("expected Box")
        };
        assert_eq!(attrs["cursor"], json!({"line":2,"column":4,"active":true}));
        assert!(children[0].text_content().contains("\u{1b}[7mH"));
    }
    #[test]
    fn highlights_filter_cursor_and_clip_to_visible_window() {
        let view = render_source_view(
            &json!({"inputState":{"renderedValue":"bcd","viewportCharOffset":1,"viewportCharEnd":4},
            "value":"abcd","showCursor":true,"cursorOffset":0,"highlights":[
                {"start":1,"end":3,"priority":1,"color":"warning"}]}),
        );
        let VNode::Element { children, .. } = view else {
            panic!("Box")
        };
        assert_eq!(children[0].text_content(), "bcd");
        assert_eq!(children[0].unresolved_node_count(), 0);
    }
    #[test]
    fn hints_are_only_for_unparameterized_slash_commands() {
        for (text, hint) in [
            ("/commit", " /message"),
            ("/commit ", "/message"),
            ("/commit hi", ""),
        ] {
            let view = render_source_view(
                &json!({"value":text,"argumentHint":"/message", "inputState":{"renderedValue":text}}),
            );
            assert_eq!(view.text_content(), format!("{text}{hint}"));
        }
    }
}
