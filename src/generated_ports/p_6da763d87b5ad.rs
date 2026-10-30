//! Original: src/components/PrBadge.tsx.
//! Preserve PR review color, terminal hyperlink, and its unlinked fallback.
use crate::generated_ports::ui::VNode;
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn text(attrs: BTreeMap<String, Value>, children: Vec<VNode>) -> VNode {
    VNode::Element {
        tag: "Text".into(),
        attrs,
        children,
    }
}

pub fn render_source_view(props: &Value) -> VNode {
    let Some(number) = props.get("number").and_then(Value::as_u64) else {
        return VNode::Empty;
    };
    let Some(url) = props.get("url").and_then(Value::as_str) else {
        return VNode::Empty;
    };
    let color = match props.get("reviewState").and_then(Value::as_str) {
        Some("approved") => Some("success"),
        Some("changes_requested") => Some("error"),
        Some("pending") => Some("warning"),
        Some("merged") => Some("merged"),
        _ => None,
    };
    let bold = props.get("bold").and_then(Value::as_bool).unwrap_or(false);
    let mut number_attrs = BTreeMap::from([
        ("dimColor".into(), json!(color.is_none() && !bold)),
        ("bold".into(), json!(bold)),
    ]);
    if let Some(color) = color {
        number_attrs.insert("color".into(), json!(color));
    }
    let number_text = format!("#{number}");
    let fallback = text(number_attrs.clone(), vec![VNode::Text(number_text.clone())]);
    number_attrs.insert("underline".into(), json!(true));
    let hyperlink = VNode::Element {
        tag: "Link".into(),
        attrs: BTreeMap::from([
            ("url".into(), json!(url)),
            (
                "fallback".into(),
                serde_json::to_value(fallback).expect("serializable fallback"),
            ),
        ]),
        children: vec![text(number_attrs, vec![VNode::Text(number_text)])],
    };
    text(
        BTreeMap::new(),
        vec![
            text(
                BTreeMap::from([("dimColor".into(), json!(!bold))]),
                vec![VNode::Text("PR".into())],
            ),
            VNode::Text(" ".into()),
            hyperlink,
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn status_and_link_match_source() {
        for (state, color) in [
            ("approved", "success"),
            ("changes_requested", "error"),
            ("pending", "warning"),
            ("merged", "merged"),
        ] {
            let badge = render_source_view(
                &json!({"number":12,"url":"https://github.com/example/repo/pull/12","reviewState":state}),
            );
            assert_eq!(badge.text_content(), "PR #12");
            assert_eq!(badge.unresolved_node_count(), 0);
            let VNode::Element { children, .. } = badge else {
                panic!("expected badge")
            };
            let VNode::Element {
                tag,
                attrs,
                children,
                ..
            } = &children[2]
            else {
                panic!("expected link")
            };
            assert_eq!(tag, "Link");
            assert_eq!(
                attrs.get("url"),
                Some(&json!("https://github.com/example/repo/pull/12"))
            );
            assert_eq!(attrs["fallback"]["kind"], "element");
            let VNode::Element { attrs, .. } = &children[0] else {
                panic!("expected linked number")
            };
            assert_eq!(attrs.get("color"), Some(&json!(color)));
            assert_eq!(attrs.get("underline"), Some(&json!(true)));
        }
    }
    #[test]
    fn absent_status_is_dim_unless_bold_and_missing_number_is_hidden() {
        let badge =
            render_source_view(&json!({"number":4,"url":"https://example.org","bold":true}));
        let VNode::Element { children, .. } = badge else {
            panic!("text")
        };
        let VNode::Element { children, .. } = &children[2] else {
            panic!("link")
        };
        let VNode::Element { attrs, .. } = &children[0] else {
            panic!("number")
        };
        assert_eq!(attrs.get("bold"), Some(&json!(true)));
        assert_eq!(attrs.get("dimColor"), Some(&json!(false)));
        assert_eq!(
            render_source_view(&json!({"url":"https://example.org"})),
            VNode::Empty
        );
    }
}
