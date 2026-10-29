//! Rust exports derived from original: src/utils/nativeInstaller/download.ts
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
pub fn ARTIFACTORY_REGISTRY_URL()->Value{serde_json::from_str(r#""https://artifactory.infra.ant.dev/artifactory/api/npm/npm-all/""#).expect("Valid source constant")}
