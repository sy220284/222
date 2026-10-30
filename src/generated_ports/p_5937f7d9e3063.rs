//! Native Rust equivalent of src/components/CustomSelect/select-option.tsx.
//! Forward the resolved props and child nodes to the design-system ListItem.
use serde_json::{json, Value};
use std::collections::BTreeMap;

use crate::generated_ports::ui::VNode;

fn as_children(node: &Value) -> Vec<VNode> {
    match node {
        Value::Null => Vec::new(),
        Value::Array(items) => items.iter().flat_map(as_children).collect(),
        Value::String(text) => vec![VNode::Text(text.clone())],
        Value::Object(_) if node.get("kind").is_some() => {
            match serde_json::from_value::<VNode>(node.clone()) {
                Ok(tree) => vec![tree],
                Err(_) => vec![VNode::Value(node.clone())],
            }
        }
        other => vec![VNode::Value(other.clone())],
    }
}

pub fn render_source_view(props: &Value) -> VNode {
    let mut attrs = BTreeMap::from([
        (
            "isFocused".into(),
            json!(props["isFocused"].as_bool().unwrap_or(false)),
        ),
        (
            "isSelected".into(),
            json!(props["isSelected"].as_bool().unwrap_or(false)),
        ),
        ("styled".into(), Value::Bool(false)),
    ]);
    for (from, to) in [
        ("description", "description"),
        ("shouldShowDownArrow", "showScrollDown"),
        ("shouldShowUpArrow", "showScrollUp"),
        ("declareCursor", "declareCursor"),
    ] {
        if let Some(value) = props.get(from) {
            attrs.insert(to.into(), value.clone());
        }
    }
    VNode::Element {
        tag: "ListItem".into(),
        attrs,
        children: as_children(&props["children"]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn option_forwards_focus_selected_scroll_description_and_cursor() {
        let tree = render_source_view(&json!({
            "isFocused":true,"isSelected":false,
            "shouldShowDownArrow":true,"shouldShowUpArrow":false,
            "description":"Help text","declareCursor":false,
            "children":[{"kind":"text","value":"4. "},{"kind":"text","value":"Configure"}]
        }));
        let VNode::Element {
            tag,
            attrs,
            children,
        } = tree
        else {
            panic!("expected ListItem")
        };
        assert_eq!(tag, "ListItem");
        assert_eq!(attrs["isFocused"], json!(true));
        assert_eq!(attrs["isSelected"], json!(false));
        assert_eq!(attrs["styled"], json!(false));
        assert_eq!(attrs["showScrollDown"], json!(true));
        assert_eq!(attrs["showScrollUp"], json!(false));
        assert_eq!(attrs["description"], json!("Help text"));
        assert_eq!(attrs["declareCursor"], json!(false));
        assert_eq!(
            children.iter().map(VNode::text_content).collect::<String>(),
            "4. Configure"
        );
    }

    #[test]
    fn default_props_do_not_introduce_unresolved_null_branches() {
        let tree = render_source_view(&json!({"children":"Test"}));
        assert_eq!(tree.text_content(), "Test");
        assert_eq!(tree.unresolved_node_count(), 0);
    }
}
