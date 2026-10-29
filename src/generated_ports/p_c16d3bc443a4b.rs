//! Rust exports derived from original: src/utils/plugins/fetchTelemetry.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
pub enum PluginFetchSource{
    #[serde(rename="install_counts")]
    install_counts,
    #[serde(rename="marketplace_clone")]
    marketplace_clone,
    #[serde(rename="marketplace_pull")]
    marketplace_pull,
    #[serde(rename="marketplace_url")]
    marketplace_url,
    #[serde(rename="plugin_clone")]
    plugin_clone,
    #[serde(rename="mcpb")]
    mcpb,
}

#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
pub enum PluginFetchOutcome{
    #[serde(rename="success")]
    success,
    #[serde(rename="failure")]
    failure,
    #[serde(rename="cache_hit")]
    cache_hit,
}
