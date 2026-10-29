//! Rust exports derived from original: src/utils/fileStateCache.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;
pub fn READ_FILE_STATE_CACHE_SIZE() -> Value {
    serde_json::from_str(r#"100"#).expect("Valid source constant")
}
