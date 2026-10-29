//! Rust exports derived from original: src/utils/model/providers.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum APIProvider {
    #[serde(rename = "firstParty")]
    firstParty,
    #[serde(rename = "bedrock")]
    bedrock,
    #[serde(rename = "vertex")]
    vertex,
    #[serde(rename = "foundry")]
    foundry,
}
