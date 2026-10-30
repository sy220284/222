//! Source: src/components/PromptInput/PromptInputStashNotice.tsx.
//! A stashed draft is shown only while the prompt input really has a stash.
use crate::generated_ports::ui::VNode;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub fn render_source_view(props: &Value) -> VNode {
    if props.get("hasStash").and_then(Value::as_bool) != Some(true) {
        return VNode::Empty;
    }
    VNode::Element {
        tag: "Box".into(),
        attrs: BTreeMap::from([("paddingLeft".into(), json!(2))]),
        children: vec![VNode::Element {
            tag: "Text".into(),
            attrs: BTreeMap::from([("dimColor".into(), json!(true))]),
            children: vec![VNode::Text("› Stashed (auto-restores after submit)".into())],
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shows_stash_only_when_flag_is_true() {
        for props in [
            json!({}),
            json!({"hasStash":false}),
            json!({"hasStash":"true"}),
        ] {
            assert_eq!(render_source_view(&props), VNode::Empty);
        }
        let node = render_source_view(&json!({"hasStash":true}));
        assert_eq!(
            node.text_content(),
            "› Stashed (auto-restores after submit)"
        );
        assert_eq!(node.unresolved_node_count(), 0);
        let VNode::Element {
            attrs, children, ..
        } = node
        else {
            panic!("box")
        };
        assert_eq!(attrs.get("paddingLeft"), Some(&json!(2)));
        let VNode::Element { attrs, .. } = &children[0] else {
            panic!("text")
        };
        assert_eq!(attrs.get("dimColor"), Some(&json!(true)));
    }
}
