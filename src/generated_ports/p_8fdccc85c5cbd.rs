//! Rust exports derived from original: src/ink/render-border.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;
pub fn CUSTOM_BORDER_STYLES() -> Value {
    serde_json::from_str(r#"{"dashed":{"top":"╌","left":"╎","right":"╎","bottom":"╌","topLeft":" ","topRight":" ","bottomLeft":" ","bottomRight":" "}}"#).expect("Valid source constant")
}
