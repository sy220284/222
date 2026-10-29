//! Rust exports derived from original: src/types/plugin.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
pub enum PluginComponent{
    #[serde(rename="commands")]
    commands,
    #[serde(rename="agents")]
    agents,
    #[serde(rename="skills")]
    skills,
    #[serde(rename="hooks")]
    hooks,
    #[serde(rename="output-styles")]
    output_styles,
}
