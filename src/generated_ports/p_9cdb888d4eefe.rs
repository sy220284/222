//! Rust exports derived from original: src/components/Spinner/teammateSelectHint.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;
pub fn TEAMMATE_SELECT_HINT() -> Value {
    serde_json::from_str(r#""shift + ↑/↓ to select""#).expect("Valid source constant")
}
