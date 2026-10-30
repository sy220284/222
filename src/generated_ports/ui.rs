//! Native virtual terminal tree. Static JSX nodes are translated into actual
//! Rust builders; property expression bindings consume runtime JSON props.
#![allow(dead_code)]
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{json, Value};
use std::collections::BTreeMap;
#[derive(Debug, Clone, PartialEq)]
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
// The original internally-tagged derive cannot serialize newtype variants
// (Text, Value, Group). Keep a stable, self-describing JSON tree, including
// the existing flat element shape, for CLI source inspection consumers.
impl Serialize for VNode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let value = match self {
            Self::Empty => json!({"kind":"empty"}),
            Self::Text(text) => json!({"kind":"text","value":text}),
            Self::Value(value) => json!({"kind":"value","value":value}),
            Self::Group(children) => json!({"kind":"group","children":children}),
            Self::Element {
                tag,
                attrs,
                children,
            } => json!({"kind":"element","tag":tag,"attrs":attrs,"children":children}),
        };
        value.serialize(serializer)
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum NodeWire {
    Empty,
    Text {
        value: String,
    },
    Value {
        value: Value,
    },
    Group {
        children: Vec<VNode>,
    },
    Element {
        tag: String,
        attrs: BTreeMap<String, Value>,
        children: Vec<VNode>,
    },
}

impl<'de> Deserialize<'de> for VNode {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match NodeWire::deserialize(deserializer)? {
            NodeWire::Empty => Self::Empty,
            NodeWire::Text { value } => Self::Text(value),
            NodeWire::Value { value } => Self::Value(value),
            NodeWire::Group { children } => Self::Group(children),
            NodeWire::Element {
                tag,
                attrs,
                children,
            } => Self::Element {
                tag,
                attrs,
                children,
            },
        })
    }
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
    fn nested_view_roundtrip_is_lossless_and_preserves_element_wire_shape() {
        let node = VNode::Element {
            tag: "Text".into(),
            attrs: BTreeMap::from([("color".into(), json!("warning"))]),
            children: vec![VNode::Text("hello".into()), VNode::Value(json!(42))],
        };
        let wire = serde_json::to_value(&node).unwrap();
        assert_eq!(wire["kind"], "element");
        assert_eq!(wire["tag"], "Text");
        assert_eq!(wire["children"][0], json!({"kind":"text","value":"hello"}));
        assert_eq!(serde_json::from_value::<VNode>(wire).unwrap(), node);
    }

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
