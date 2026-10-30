//! Rust exports derived from original: src/utils/plugins/addDirPluginSettings.ts
//!
//! Full equivalence at the merge layer: within a directory settings.local.json
//! wins over settings.json; across --add-dir directories later CLI order wins.
//! The result has the LOWEST priority — callers spread standard settings on
//! top (user/project/local/flag/policy all override).

/// `SETTINGS_FILES` — processed in this order per directory.
pub const SETTINGS_FILES: &[&str] = &["settings.json", "settings.local.json"];

use std::collections::BTreeMap;

/// Merge enabledPlugins maps in processing order; later entries win.
pub fn merge_enabled_plugins(maps: &[BTreeMap<String, bool>]) -> BTreeMap<String, bool> {
    let mut out = BTreeMap::new();
    for m in maps {
        for (k, v) in m {
            out.insert(k.clone(), *v);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn later_wins() {
        let mut a = BTreeMap::new();
        a.insert("p1".to_owned(), true);
        let mut b = BTreeMap::new();
        b.insert("p1".to_owned(), false);
        let merged = merge_enabled_plugins(&[a, b]);
        assert_eq!(merged.get("p1"), Some(&false));
    }
}
