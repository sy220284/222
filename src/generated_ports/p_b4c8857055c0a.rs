//! Rust exports derived from original: src/utils/bash/parser.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct ParsedCommandData{
    #[serde(rename="rootNode")]
    pub rootNode:Value,
    #[serde(rename="envVars")]
    pub envVars:Vec<String>,
    #[serde(rename="commandNode")]
    pub commandNode:Value,
    #[serde(rename="originalCommand")]
    pub originalCommand:String,
}
