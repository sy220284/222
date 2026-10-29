//! Rust exports derived from original: src/components/ScrollKeybindingHandler.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ModalPagerAction {
    #[serde(rename = "lineUp")]
    lineUp,
    #[serde(rename = "lineDown")]
    lineDown,
    #[serde(rename = "halfPageUp")]
    halfPageUp,
    #[serde(rename = "halfPageDown")]
    halfPageDown,
    #[serde(rename = "fullPageUp")]
    fullPageUp,
    #[serde(rename = "fullPageDown")]
    fullPageDown,
    #[serde(rename = "top")]
    top,
    #[serde(rename = "bottom")]
    bottom,
}

use serde_json::json;

pub fn initWheelAccel(args: &[Value]) -> Value {
    {
        let mut map = serde_json::Map::new();
        map.insert("time".into(), json!(0));
        map.insert("mult".into(), args.get(1).cloned().unwrap_or(Value::Null));
        map.insert("dir".into(), json!(0));
        map.insert(
            "xtermJs".into(),
            args.get(0).cloned().unwrap_or(Value::Null),
        );
        map.insert("frac".into(), json!(0));
        map.insert("base".into(), args.get(1).cloned().unwrap_or(Value::Null));
        map.insert("pendingFlip".into(), Value::Bool(false));
        map.insert("wheelMode".into(), Value::Bool(false));
        map.insert("burstCount".into(), json!(0));
        Value::Object(map)
    }
}
