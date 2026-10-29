//! Native virtual terminal tree. Static JSX nodes are translated into actual
//! Rust builders; property expression bindings consume runtime JSON props.
#![allow(dead_code)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
use std::collections::BTreeMap;
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq)]
#[serde(tag="kind",rename_all="snake_case")]
pub enum VNode{
    Empty,
    Text(String),
    Value(Value),
    Group(Vec<VNode>),
    Element{tag:String,attrs:BTreeMap<String,Value>,children:Vec<VNode>},
}
impl VNode{
    pub fn text_content(&self)->String{match self{
        Self::Empty=>String::new(),Self::Text(s)=>s.clone(),
        Self::Value(v)=>match v{Value::String(s)=>s.clone(),Value::Null=>String::new(),x=>x.to_string()},
        Self::Group(a)=>a.iter().map(Self::text_content).collect(),
        Self::Element{children,..}=>children.iter().map(Self::text_content).collect(),
    }}
    pub fn node_count(&self)->usize{match self{
        Self::Empty=>0,Self::Text(_)|Self::Value(_)=>1,
        Self::Group(a)=>a.iter().map(Self::node_count).sum(),
        Self::Element{children,..}=>1+children.iter().map(Self::node_count).sum::<usize>(),
    }}
}