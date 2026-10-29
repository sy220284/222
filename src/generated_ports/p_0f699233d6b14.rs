//! Rust exports derived from original: src/utils/theme.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;
pub fn THEME_NAMES() -> Value {
    serde_json::from_str(
        r#"["dark","light","light-daltonized","dark-daltonized","light-ansi","dark-ansi"]"#,
    )
    .expect("Valid source constant")
}
