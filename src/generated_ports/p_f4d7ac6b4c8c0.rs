//! Rust exports derived from original: src/vim/types.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Operator {
    #[serde(rename = "delete")]
    delete,
    #[serde(rename = "change")]
    change,
    #[serde(rename = "yank")]
    yank,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FindType {
    #[serde(rename = "f")]
    f,
    #[serde(rename = "F")]
    F,
    #[serde(rename = "t")]
    t,
    #[serde(rename = "T")]
    T,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TextObjScope {
    #[serde(rename = "inner")]
    inner,
    #[serde(rename = "around")]
    around,
}

pub fn OPERATORS() -> Value {
    serde_json::from_str(r#"{"d":"delete","c":"change","y":"yank"}"#)
        .expect("Valid source constant")
}

pub fn TEXT_OBJ_SCOPES() -> Value {
    serde_json::from_str(r#"{"i":"inner","a":"around"}"#).expect("Valid source constant")
}

pub fn MAX_VIM_COUNT() -> Value {
    serde_json::from_str(r#"10000"#).expect("Valid source constant")
}

pub fn createInitialVimState(args: &[Value]) -> Value {
    {
        let mut map = serde_json::Map::new();
        map.insert("mode".into(), Value::String("INSERT".into()));
        map.insert("insertedText".into(), Value::String("".into()));
        Value::Object(map)
    }
}
pub fn createInitialPersistentState(args: &[Value]) -> Value {
    {
        let mut map = serde_json::Map::new();
        map.insert("lastChange".into(), Value::Null);
        map.insert("lastFind".into(), Value::Null);
        map.insert("register".into(), Value::String("".into()));
        map.insert("registerIsLinewise".into(), Value::Bool(false));
        Value::Object(map)
    }
}
