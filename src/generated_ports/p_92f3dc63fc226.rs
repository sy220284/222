//! Native Rust port derived from original: src/utils/secureStorage/index.ts
//!
//! Platform-appropriate secure storage selector. The original picks
//! macOS Keychain first and falls back to plaintext; the repository
//! scope excludes vendor-specific keychain access (see env_runtime
//! notes and authPortable commentary), so both branches resolve to the
//! plaintext-backed fallback chain. The macOS arm is kept as a
//! documented hook for a future security-framework port.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use std::sync::Arc;

use crate::plain_text_storage::PlainTextStorage;
use crate::secure_fallback::{FallbackStorage, SecureStorage};

/// Returns the platform-appropriate secure storage.
pub fn get_secure_storage() -> Arc<dyn SecureStorage> {
    #[cfg(target_os = "macos")]
    {
        // macOS: vendor Keychain storage is out of equivalence scope;
        // the fallback chain still prefers it once a security-framework
        // port lands (hook kept for that future port).
    }
    let home = crate::env_runtime::config_home_dir(None).unwrap_or_default();
    Arc::new(FallbackStorage::new(
        Arc::new(PlainTextStorage::new(home.clone())),
        Arc::new(PlainTextStorage::new(home)),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_is_named_and_round_trips() {
        let storage = get_secure_storage();
        assert!(!storage.name().is_empty());
        let result = storage.update(serde_json::json!({"OAuth": {"token": "t"}}));
        assert!(result.success);
        assert!(storage.read().is_some());
        assert!(storage.delete());
        // Fallback chain reads never surface null: empty object after delete.
        assert_eq!(storage.read(), Some(serde_json::json!({})));
    }
}
