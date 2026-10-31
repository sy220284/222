//! Rust exports derived from original: src/ink/hooks/use-search-highlight.ts
//!
//! Search highlight contract: setQuery paints every visible occurrence
//! inverted (SGR 7, screen-buffer overlay, same damage machinery as
//! selection); setPositions overlays the yellow current-match marker per
//! frame at positions[currentIdx] + rowOffset. The highlight is screen-space
//! — it matches RENDERED text, so source-visible but render-truncated
//! matches do not highlight (original note kept verbatim in spirit).

/// Highlight surface names, for the ink runtime host to implement.
pub const SET_QUERY: &str = "setQuery";
pub const SCAN_ELEMENT: &str = "scanElement";
pub const SET_POSITIONS: &str = "setPositions";
