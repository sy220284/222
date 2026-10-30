//! Rust exports derived from original: src/commands/thinkback-play/thinkback-play.ts
//!
//! Brand-adjacent skill command (thinkback animation from the marketplace
//! plugin). Structure ported; the internal marketplace name is a vendor
//! constant kept neutral here, and the animation loader rides the plugin
//! manager chain (loadInstalledPluginsV2).

/// `getPluginId` — skill id under the active marketplace, ant internal vs
/// official marketplace selected by the build define.
pub fn plugin_id(user_type_is_ant: bool, official_marketplace: &str) -> String {
    let marketplace = if user_type_is_ant {
        "internal-marketplace"
    } else {
        official_marketplace
    };
    format!("thinkback@{marketplace}")
}

/// Missing-installation result copy (exact original text path).
pub fn missing_installation_text() -> &'static str {
    "Thinkback skill not found. Install it from the marketplace."
}
