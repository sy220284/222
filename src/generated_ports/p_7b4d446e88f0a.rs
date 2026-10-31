//! Rust exports derived from original: src/services/lsp/config.ts
//!
//! Merge contract: LSP servers come only from plugins (never user/project
//! settings); per-plugin results merge in original order so later plugins
//! win collisions (Object.assign precedence). I/O rides the plugin loader.

use std::collections::BTreeMap;

/// Merge plugin LSP server maps in original order (later wins).
pub fn merge_lsp_servers(ordered: Vec<BTreeMap<String, String>>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for map in ordered {
        for (k, v) in map {
            out.insert(k, v);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn later_plugins_win() {
        let mut a = BTreeMap::new();
        a.insert("rust".into(), "a".into());
        let mut b = BTreeMap::new();
        b.insert("rust".into(), "b".into());
        b.insert("go".into(), "b".into());
        let merged = merge_lsp_servers(vec![a, b]);
        assert_eq!(merged.get("rust").unwrap(), "b");
        assert_eq!(merged.get("go").unwrap(), "b");
    }
}
