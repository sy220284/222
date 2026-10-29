//! Rust exports derived from original: src/ink/selection.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
pub enum FocusMove{
    #[serde(rename="left")]
    left,
    #[serde(rename="right")]
    right,
    #[serde(rename="up")]
    up,
    #[serde(rename="down")]
    down,
    #[serde(rename="lineStart")]
    lineStart,
    #[serde(rename="lineEnd")]
    lineEnd,
}

use serde_json::json;

pub fn createSelectionState(args:&[Value])->Value {{let mut map=serde_json::Map::new();map.insert("anchor".into(),Value::Null);map.insert("focus".into(),Value::Null);map.insert("isDragging".into(),Value::Bool(false));map.insert("anchorSpan".into(),Value::Null);map.insert("scrolledOffAbove".into(),Value::Array(vec![]));map.insert("scrolledOffBelow".into(),Value::Array(vec![]));map.insert("scrolledOffAboveSW".into(),Value::Array(vec![]));map.insert("scrolledOffBelowSW".into(),Value::Array(vec![]));map.insert("lastPressHadAlt".into(),Value::Bool(false)); Value::Object(map)}}
pub fn hasSelection(args:&[Value])->Value {super::runtime::and(&super::runtime::strict_unequal(&super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null),"anchor"),&Value::Null),&super::runtime::strict_unequal(&super::runtime::property(&args.get(0).cloned().unwrap_or(Value::Null),"focus"),&Value::Null))}
