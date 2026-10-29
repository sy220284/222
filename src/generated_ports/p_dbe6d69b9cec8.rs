//! Rust exports derived from original: src/ink/layout/node.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
pub fn LayoutEdge()->Value{serde_json::from_str(r#"{"All":"all","Horizontal":"horizontal","Vertical":"vertical","Left":"left","Right":"right","Top":"top","Bottom":"bottom","Start":"start","End":"end"}"#).expect("Valid source constant")}

pub fn LayoutGutter()->Value{serde_json::from_str(r#"{"All":"all","Column":"column","Row":"row"}"#).expect("Valid source constant")}

pub fn LayoutDisplay()->Value{serde_json::from_str(r#"{"Flex":"flex","None":"none"}"#).expect("Valid source constant")}

pub fn LayoutFlexDirection()->Value{serde_json::from_str(r#"{"Row":"row","RowReverse":"row-reverse","Column":"column","ColumnReverse":"column-reverse"}"#).expect("Valid source constant")}

pub fn LayoutAlign()->Value{serde_json::from_str(r#"{"Auto":"auto","Stretch":"stretch","FlexStart":"flex-start","Center":"center","FlexEnd":"flex-end"}"#).expect("Valid source constant")}

pub fn LayoutJustify()->Value{serde_json::from_str(r#"{"FlexStart":"flex-start","Center":"center","FlexEnd":"flex-end","SpaceBetween":"space-between","SpaceAround":"space-around","SpaceEvenly":"space-evenly"}"#).expect("Valid source constant")}

pub fn LayoutWrap()->Value{serde_json::from_str(r#"{"NoWrap":"nowrap","Wrap":"wrap","WrapReverse":"wrap-reverse"}"#).expect("Valid source constant")}

pub fn LayoutPositionType()->Value{serde_json::from_str(r#"{"Relative":"relative","Absolute":"absolute"}"#).expect("Valid source constant")}

pub fn LayoutOverflow()->Value{serde_json::from_str(r#"{"Visible":"visible","Hidden":"hidden","Scroll":"scroll"}"#).expect("Valid source constant")}

pub fn LayoutMeasureMode()->Value{serde_json::from_str(r#"{"Undefined":"undefined","Exactly":"exactly","AtMost":"at-most"}"#).expect("Valid source constant")}
