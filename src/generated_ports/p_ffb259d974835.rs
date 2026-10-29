//! Rust exports derived from original: src/components/permissions/FilePermissionDialog/ideDiffConfig.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEdit {
    #[serde(rename = "old_string")]
    pub old_string: String,
    #[serde(rename = "new_string")]
    pub new_string: String,
    #[serde(rename = "replace_all", default)]
    pub replace_all: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IDEDiffConfig {
    #[serde(rename = "filePath")]
    pub filePath: String,
    #[serde(rename = "edits", default)]
    pub edits: Option<Vec<Value>>,
    #[serde(rename = "editMode", default)]
    pub editMode: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IDEDiffChangeInput {
    #[serde(rename = "file_path")]
    pub file_path: String,
    #[serde(rename = "edits")]
    pub edits: Vec<Value>,
}

use serde_json::json;

pub fn createSingleEditDiffConfig(args: &[Value]) -> Value {
    {
        let mut map = serde_json::Map::new();
        map.insert(
            "filePath".into(),
            args.get(0).cloned().unwrap_or(Value::Null),
        );
        map.insert(
            "edits".into(),
            Value::Array(vec![{
                let mut map = serde_json::Map::new();
                map.insert(
                    "old_string".into(),
                    args.get(1).cloned().unwrap_or(Value::Null),
                );
                map.insert(
                    "new_string".into(),
                    args.get(2).cloned().unwrap_or(Value::Null),
                );
                map.insert(
                    "replace_all".into(),
                    args.get(3).cloned().unwrap_or(Value::Null),
                );
                Value::Object(map)
            }]),
        );
        map.insert("editMode".into(), Value::String("single".into()));
        Value::Object(map)
    }
}
