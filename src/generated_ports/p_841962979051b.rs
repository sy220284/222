//! Rust exports derived from original: src/components/TeleportError.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TeleportLocalErrorType {
    #[serde(rename = "needsLogin")]
    needsLogin,
    #[serde(rename = "needsGitStash")]
    needsGitStash,
}
