//! Rust exports derived from original: src/utils/secureStorage/macOsKeychainHelpers.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
pub fn CREDENTIALS_SERVICE_SUFFIX()->Value{serde_json::from_str(r#""-credentials""#).expect("Valid source constant")}

pub fn KEYCHAIN_CACHE_TTL_MS()->Value{serde_json::from_str(r#"30000"#).expect("Valid source constant")}

pub fn keychainCacheState()->Value{serde_json::from_str(r#"{"cache":{"data":null,"cachedAt":0},"generation":0,"readInFlight":null}"#).expect("Valid source constant")}
