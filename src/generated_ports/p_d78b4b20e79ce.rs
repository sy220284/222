//! Hand-port of src/components/design-system/Byline.tsx.
//! React.Children.toArray discards nullish and boolean children, then inserts
//! one dim middle-dot between each retained child.
#![allow(dead_code)]
use crate::generated_ports::ui::VNode;
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn children_from_json(value: &Value, result: &mut Vec<VNode>) {
    match value {
        Value::Null | Value::Bool(_) => {}
        Value::Array(elements) => {
            for child in elements {
                children_from_json(child, result);
            }
        }
        Value::Object(_) => {
            let child = serde_json::from_value::<VNode>(value.clone())
                .expect("object React child must be a serialized VNode");
            result.push(child);
        }
        Value::String(s) => result.push(VNode::Text(s.clone())),
        Value::Number(n) => result.push(VNode::Text(n.to_string())),
    }
}

pub fn render_source_view(props: &Value) -> VNode {
    let mut children = Vec::new();
    children_from_json(props.get("children").unwrap_or(&Value::Null), &mut children);
    if children.is_empty() {
        return VNode::Empty;
    }
    let mut joined = Vec::with_capacity(children.len() * 2 - 1);
    for (i, child) in children.into_iter().enumerate() {
        if i > 0 {
            joined.push(VNode::Element {
                tag: "Text".into(),
                attrs: BTreeMap::from([("dimColor".into(), json!(true))]),
                children: vec![VNode::Text(" · ".into())],
            });
        }
        joined.push(child);
    }
    VNode::Group(joined)
}

#[cfg(test)]
mod source_parity_tests {
    use super::*;
    #[test]
    fn filters_conditional_children_and_joins_with_single_separator() {
        let node = render_source_view(
            &json!({"children": [null,false,"Enter to confirm",null,true,"Esc to cancel"]}),
        );
        assert_eq!(node.text_content(), "Enter to confirm · Esc to cancel");
        assert_eq!(node.unresolved_node_count(), 0);
        assert_eq!(
            render_source_view(&json!({"children":[null,false]})),
            VNode::Empty
        );
    }
    #[test]
    fn supports_actual_nested_ui_children() {
        let child = VNode::Element {
            tag: "Text".into(),
            attrs: BTreeMap::new(),
            children: vec![VNode::Text("item".into())],
        };
        let node = render_source_view(&json!({"children":[child,"other"]}));
        assert_eq!(node.text_content(), "item · other");
    }
}
