//! Rust exports derived from original:
//! src/migrations/migrateReplBridgeEnabledToRemoteControlAtStartup.ts
//!
//! Full equivalence: the old `replBridgeEnabled` implementation-detail key
//! migrates to `remoteControlAtStartup` — copies the value, removes the old
//! key, idempotent (skips when the old key is absent or the new one set).

use serde_json::{json, Map, Value};

/// Pure decision core over a config map; the persistence wrapper (saveGlobalConfig)
/// rides the settings chain.
pub fn migrate_repl_bridge_enabled(config: &mut Map<String, Value>) -> bool {
    let Some(old) = config.get("replBridgeEnabled").cloned() else {
        return false;
    };
    if config
        .get("remoteControlAtStartup")
        .is_some_and(|v| !v.is_null())
    {
        return false;
    }
    config.insert(
        "remoteControlAtStartup".into(),
        json!(old.as_bool().unwrap_or(false)),
    );
    config.remove("replBridgeEnabled");
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_then_removes_and_is_idempotent() {
        let mut cfg = Map::new();
        cfg.insert("replBridgeEnabled".into(), json!(true));
        assert!(migrate_repl_bridge_enabled(&mut cfg));
        assert_eq!(cfg.get("remoteControlAtStartup"), Some(&json!(true)));
        assert!(cfg.get("replBridgeEnabled").is_none());
        // second run: old key gone → no-op
        assert!(!migrate_repl_bridge_enabled(&mut cfg));
    }

    #[test]
    fn skips_when_new_key_already_set() {
        let mut cfg = Map::new();
        cfg.insert("replBridgeEnabled".into(), json!(true));
        cfg.insert("remoteControlAtStartup".into(), json!(false));
        assert!(!migrate_repl_bridge_enabled(&mut cfg));
        assert_eq!(cfg.get("remoteControlAtStartup"), Some(&json!(false)));
    }
}
