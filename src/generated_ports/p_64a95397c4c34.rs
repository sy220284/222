//! Rust exports derived from original: src/utils/model/agent.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::{json, Value};

pub fn getDefaultSubagentModel(args: &[Value]) -> Value {
    Value::String("inherit".into())
}
pub fn getAgentModelOptions(args: &[Value]) -> Value {
    Value::Array(vec![
        {
            let mut map = serde_json::Map::new();
            map.insert("value".into(), Value::String("sonnet".into()));
            map.insert("label".into(), Value::String("Sonnet".into()));
            map.insert(
                "description".into(),
                Value::String("Balanced performance - best for most agents".into()),
            );
            Value::Object(map)
        },
        {
            let mut map = serde_json::Map::new();
            map.insert("value".into(), Value::String("opus".into()));
            map.insert("label".into(), Value::String("Opus".into()));
            map.insert(
                "description".into(),
                Value::String("Most capable for complex reasoning tasks".into()),
            );
            Value::Object(map)
        },
        {
            let mut map = serde_json::Map::new();
            map.insert("value".into(), Value::String("haiku".into()));
            map.insert("label".into(), Value::String("Haiku".into()));
            map.insert(
                "description".into(),
                Value::String("Fast and efficient for simple tasks".into()),
            );
            Value::Object(map)
        },
        {
            let mut map = serde_json::Map::new();
            map.insert("value".into(), Value::String("inherit".into()));
            map.insert("label".into(), Value::String("Inherit from parent".into()));
            map.insert(
                "description".into(),
                Value::String("Use the same model as the main conversation".into()),
            );
            Value::Object(map)
        },
    ])
}
