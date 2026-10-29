//! Original: src/components/InterruptedByUser.tsx.
//! The upstream external-build alternative is the active branch; the
//! vendor-specific name is replaced with the provider-neutral assistant.
use crate::generated_ports::ui::VNode;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub fn render_source_view(_props: &Value) -> VNode {
    let attrs = BTreeMap::from([("dimColor".into(), json!(true))]);
    VNode::Group(vec![
        VNode::Element {
            tag: "Text".into(),
            attrs: attrs.clone(),
            children: vec![VNode::Text("Interrupted ".into())],
        },
        VNode::Element {
            tag: "Text".into(),
            attrs,
            children: vec![VNode::Text(
                "· What should the assistant do instead?".into(),
            )],
        },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn external_interrupt_message_keeps_both_visible_text_nodes() {
        let view = render_source_view(&json!({}));
        assert_eq!(
            view.text_content(),
            "Interrupted · What should the assistant do instead?"
        );
        assert_eq!(view.unresolved_node_count(), 0);
        let VNode::Group(children) = view else {
            panic!("expected group")
        };
        assert_eq!(children.len(), 2);
        assert!(children.iter().all(|node| matches!(node,VNode::Element{attrs,..} if attrs.get("dimColor")==Some(&json!(true)))));
    }
}
