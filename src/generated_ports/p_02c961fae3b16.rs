//! Source-aligned tool-state indicator from src/components/ToolUseLoader.tsx.
//! The host supplies the synchronized blink clock's visibility; a static
//! inspection renders the initial visible state, like useBlink's initial tick.
use crate::generated_ports::ui::VNode;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub fn render_source_view(props: &Value) -> VNode {
    let Some(error) = props.get("isError").and_then(Value::as_bool) else {
        return VNode::Empty;
    };
    let Some(unresolved) = props.get("isUnresolved").and_then(Value::as_bool) else {
        return VNode::Empty;
    };
    let Some(animate) = props.get("shouldAnimate").and_then(Value::as_bool) else {
        return VNode::Empty;
    };
    let blinking = props
        .get("isBlinking")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let shown = !animate || blinking || error || !unresolved;
    let glyph = if cfg!(target_os = "macos") {
        "⏺"
    } else {
        "●"
    };
    let mut attrs = BTreeMap::from([("dimColor".into(), json!(unresolved))]);
    if !unresolved {
        attrs.insert(
            "color".into(),
            json!(if error { "error" } else { "success" }),
        );
    }
    VNode::Element {
        tag: "Box".into(),
        attrs: BTreeMap::from([("minWidth".into(), json!(2))]),
        children: vec![VNode::Element {
            tag: "Text".into(),
            attrs,
            children: vec![VNode::Text(if shown { glyph } else { " " }.into())],
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn status_colors_and_blink_visibility_follow_source() {
        for (error, unresolved, animate, blink, visible, color) in [
            (false, false, true, false, true, Some("success")),
            (true, false, true, false, true, Some("error")),
            (false, true, true, false, false, None),
            (false, true, true, true, true, None),
            (false, true, false, false, true, None),
        ] {
            let node = render_source_view(
                &json!({"isError":error,"isUnresolved":unresolved,"shouldAnimate":animate,"isBlinking":blink}),
            );
            let VNode::Element {
                attrs, children, ..
            } = node
            else {
                panic!("box")
            };
            assert_eq!(attrs.get("minWidth"), Some(&json!(2)));
            let VNode::Element {
                attrs, children, ..
            } = &children[0]
            else {
                panic!("dot")
            };
            assert_eq!(attrs.get("color").and_then(Value::as_str), color);
            assert_eq!(attrs.get("dimColor"), Some(&json!(unresolved)));
            assert_eq!(children[0].text_content() == " ", !visible);
        }
        assert_eq!(render_source_view(&json!({"isError":false})), VNode::Empty);
    }
}
