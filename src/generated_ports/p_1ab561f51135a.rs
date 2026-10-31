//! Rust exports derived from original:
//! src/components/permissions/PermissionRequestTitle.tsx
//!
//! Title decision core: bold title in the given color (default
//! "permission"); the optional subtitle renders between title and badge.
//! Worker badge layout belongs to the ink host.

/// Effective title color.
pub fn title_color(override_color: Option<&str>) -> &str {
    override_color.unwrap_or("permission")
}

/// Whether the subtitle block renders.
pub fn has_subtitle(subtitle: Option<&str>) -> bool {
    subtitle.is_some()
}
