//! Rust exports derived from original: src/ink/termio/types.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NamedColor {
    #[serde(rename = "black")]
    black,
    #[serde(rename = "red")]
    red,
    #[serde(rename = "green")]
    green,
    #[serde(rename = "yellow")]
    yellow,
    #[serde(rename = "blue")]
    blue,
    #[serde(rename = "magenta")]
    magenta,
    #[serde(rename = "cyan")]
    cyan,
    #[serde(rename = "white")]
    white,
    #[serde(rename = "brightBlack")]
    brightBlack,
    #[serde(rename = "brightRed")]
    brightRed,
    #[serde(rename = "brightGreen")]
    brightGreen,
    #[serde(rename = "brightYellow")]
    brightYellow,
    #[serde(rename = "brightBlue")]
    brightBlue,
    #[serde(rename = "brightMagenta")]
    brightMagenta,
    #[serde(rename = "brightCyan")]
    brightCyan,
    #[serde(rename = "brightWhite")]
    brightWhite,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum UnderlineStyle {
    #[serde(rename = "none")]
    none,
    #[serde(rename = "single")]
    single,
    #[serde(rename = "double")]
    double,
    #[serde(rename = "curly")]
    curly,
    #[serde(rename = "dotted")]
    dotted,
    #[serde(rename = "dashed")]
    dashed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CursorDirection {
    #[serde(rename = "up")]
    up,
    #[serde(rename = "down")]
    down,
    #[serde(rename = "forward")]
    forward,
    #[serde(rename = "back")]
    back,
}

pub fn defaultStyle(args: &[Value]) -> Value {
    {
        let mut map = serde_json::Map::new();
        map.insert("bold".into(), Value::Bool(false));
        map.insert("dim".into(), Value::Bool(false));
        map.insert("italic".into(), Value::Bool(false));
        map.insert("underline".into(), Value::String("none".into()));
        map.insert("blink".into(), Value::Bool(false));
        map.insert("inverse".into(), Value::Bool(false));
        map.insert("hidden".into(), Value::Bool(false));
        map.insert("strikethrough".into(), Value::Bool(false));
        map.insert("overline".into(), Value::Bool(false));
        map.insert("fg".into(), {
            let mut map = serde_json::Map::new();
            map.insert("type".into(), Value::String("default".into()));
            Value::Object(map)
        });
        map.insert("bg".into(), {
            let mut map = serde_json::Map::new();
            map.insert("type".into(), Value::String("default".into()));
            Value::Object(map)
        });
        map.insert("underlineColor".into(), {
            let mut map = serde_json::Map::new();
            map.insert("type".into(), Value::String("default".into()));
            Value::Object(map)
        });
        Value::Object(map)
    }
}
