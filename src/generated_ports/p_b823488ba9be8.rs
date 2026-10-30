//! Rust exports derived from original: src/components/Spinner/index.ts
//!
//! Pure re-export surface: FlashingChar, GlimmerMessage, ShimmerChar,
//! SpinnerGlyph, SpinnerMode, useShimmerAnimation, useStalledAnimation,
//! getDefaultCharacters, interpolateColor. Teammate components are NOT
//! exported here — the original keeps a dynamic require() for dead code
//! elimination (note preserved from the source).
pub const RE_EXPORT_SURFACE: &[&str] = &[
    "FlashingChar",
    "GlimmerMessage",
    "ShimmerChar",
    "SpinnerGlyph",
    "SpinnerMode",
    "useShimmerAnimation",
    "useStalledAnimation",
    "getDefaultCharacters",
    "interpolateColor",
];

/// Teammate components load dynamically (original comment preserved).
pub const TEAMMATE_DYNAMIC_REQUIRE: bool = true;
