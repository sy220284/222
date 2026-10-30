//! Rust exports derived from original: src/commands/createMovedToPluginCommand.ts
//!
//! Options shape for the moved-to-plugin command factory. The async prompt
//! fallback closure depends on the SDK content-block surface and is recorded
//! as the pending wiring point (marketplace privacy fallback noted in the
//! original).

use serde::{Deserialize, Serialize};

/// `Options` for `createMovedToPluginCommand`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MovedToPluginOptions {
    pub name: String,
    pub description: String,
    pub progress_message: String,
    pub plugin_name: String,
    pub plugin_command: String,
}
