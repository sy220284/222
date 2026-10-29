//! Rust exports derived from original: src/utils/env.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;
pub fn JETBRAINS_IDES() -> Value {
    serde_json::from_str(r#"["pycharm","intellij","webstorm","phpstorm","rubymine","clion","goland","rider","datagrip","appcode","dataspell","aqua","gateway","fleet","jetbrains","androidstudio"]"#).expect("Valid source constant")
}
