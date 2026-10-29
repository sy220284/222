//! Rust exports derived from original: src/utils/imageResizer.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct ResizeResult{
    #[serde(rename="buffer")]
    pub buffer:Value,
    #[serde(rename="mediaType")]
    pub mediaType:String,
    #[serde(rename="dimensions",default)]
    pub dimensions:Option<Value>,
}

#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct ImageBlockWithDimensions{
    #[serde(rename="block")]
    pub block:Value,
    #[serde(rename="dimensions",default)]
    pub dimensions:Option<Value>,
}
