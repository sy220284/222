//! Rust exports derived from original: src/ink/termio/csi.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub fn CSI_RANGE() -> Value {
    serde_json::from_str(r#"{"PARAM_START":48,"PARAM_END":63,"INTERMEDIATE_START":32,"INTERMEDIATE_END":47,"FINAL_START":64,"FINAL_END":126}"#).expect("Valid source constant")
}

pub fn CSI() -> Value {
    serde_json::from_str(r#"{"CUU":65,"CUD":66,"CUF":67,"CUB":68,"CNL":69,"CPL":70,"CHA":71,"CUP":72,"CHT":73,"VPA":100,"HVP":102,"ED":74,"EL":75,"ECH":88,"IL":76,"DL":77,"ICH":64,"DCH":80,"SU":83,"SD":84,"SM":104,"RM":108,"SGR":109,"DSR":110,"DECSCUSR":113,"DECSTBM":114,"SCOSC":115,"SCORC":117,"CBT":90}"#).expect("Valid source constant")
}

pub fn ERASE_DISPLAY() -> Value {
    serde_json::from_str(r#"["toEnd","toStart","all","scrollback"]"#)
        .expect("Valid source constant")
}

pub fn ERASE_LINE_REGION() -> Value {
    serde_json::from_str(r#"["toEnd","toStart","all"]"#).expect("Valid source constant")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CursorStyle {
    #[serde(rename = "block")]
    block,
    #[serde(rename = "underline")]
    underline,
    #[serde(rename = "bar")]
    bar,
}

pub fn CURSOR_STYLES() -> Value {
    serde_json::from_str(r#"[{"style":"block","blinking":true},{"style":"block","blinking":true},{"style":"block","blinking":false},{"style":"underline","blinking":true},{"style":"underline","blinking":false},{"style":"bar","blinking":true},{"style":"bar","blinking":false}]"#).expect("Valid source constant")
}
