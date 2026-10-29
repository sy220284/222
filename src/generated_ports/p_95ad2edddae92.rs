//! Rust exports derived from original: src/ink/supports-hyperlinks.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
pub fn ADDITIONAL_HYPERLINK_TERMINALS()->Value{serde_json::from_str(r#"["ghostty","Hyper","kitty","alacritty","iTerm.app","iTerm2"]"#).expect("Valid source constant")}
