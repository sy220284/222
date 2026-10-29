//! Rust exports derived from original: src/utils/managedEnvConstants.ts
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub fn DANGEROUS_SHELL_SETTINGS() -> Value {
    serde_json::from_str(r#"["apiKeyHelper","awsAuthRefresh","awsCredentialExport","gcpAuthRefresh","otelHeadersHelper","statusLine"]"#).expect("Valid source constant")
}
