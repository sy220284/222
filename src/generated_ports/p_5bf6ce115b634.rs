//! Rust exports derived from original: src/services/voiceStreamSTT.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
pub fn FINALIZE_TIMEOUTS_MS()->Value{serde_json::from_str(r#"{"safety":5000,"noData":1500}"#).expect("Valid source constant")}

#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
pub enum FinalizeSource{
    #[serde(rename="post_closestream_endpoint")]
    post_closestream_endpoint,
    #[serde(rename="no_data_timeout")]
    no_data_timeout,
    #[serde(rename="safety_timeout")]
    safety_timeout,
    #[serde(rename="ws_close")]
    ws_close,
    #[serde(rename="ws_already_closed")]
    ws_already_closed,
}
