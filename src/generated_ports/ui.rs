//! Native virtual terminal tree. Static JSX nodes are translated into actual
//! Rust builders; property expression bindings consume runtime JSON props.
#![allow(dead_code)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VNode {
    Empty,
    Text(String),
    Value(Value),
    Group(Vec<VNode>),
    Element {
        tag: String,
        attrs: BTreeMap<String, Value>,
        children: Vec<VNode>,
    },
}
impl VNode {
    pub fn text_content(&self) -> String {
        match self {
            Self::Empty => String::new(),
            Self::Text(s) => s.clone(),
            Self::Value(v) => match v {
                Value::String(s) => s.clone(),
                Value::Null => String::new(),
                x => x.to_string(),
            },
            Self::Group(a) => a.iter().map(Self::text_content).collect(),
            Self::Element { children, .. } => children.iter().map(Self::text_content).collect(),
        }
    }
    /// Count unresolved expressions which the old AST-only JSX converter
    /// substituted with JSON null. These must not be presented as finished UI.
    pub fn unresolved_node_count(&self) -> usize {
        match self {
            Self::Empty | Self::Text(_) => 0,
            Self::Value(Value::Null) => 1,
            Self::Value(_) => 0,
            Self::Group(children) => children.iter().map(Self::unresolved_node_count).sum(),
            Self::Element {
                attrs, children, ..
            } => {
                attrs.values().filter(|v| v.is_null()).count()
                    + children
                        .iter()
                        .map(Self::unresolved_node_count)
                        .sum::<usize>()
            }
        }
    }
    pub fn node_count(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::Text(_) | Self::Value(_) => 1,
            Self::Group(a) => a.iter().map(Self::node_count).sum(),
            Self::Element { children, .. } => {
                1 + children.iter().map(Self::node_count).sum::<usize>()
            }
        }
    }
}

#[cfg(test)]
mod conversion_audit_tests {
    use super::*;
    #[test]
    fn unresolved_jsx_does_not_silently_look_complete() {
        let node = VNode::Group(vec![
            VNode::Value(Value::Null),
            VNode::Element {
                tag: "Text".into(),
                attrs: BTreeMap::from([("color".into(), Value::Null)]),
                children: vec![VNode::Text("Hello".into())],
            },
        ]);
        assert_eq!(node.unresolved_node_count(), 2);
        assert_eq!(node.text_content(), "Hello");
    }
}
