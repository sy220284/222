//! Rust exports derived from original: src/ink/termio.ts
//!
//! Pure re-export module: Parser plus the action/color style types. The
//! underlying termio/{parser,ansi,csi,dec,osc} sources each have their own
//! generated port stubs (p_1f1e68eb35633, p_bc5aed2cde71b, p_5753cc2fee99d,
//! ...); this module records the public surface and forwards nothing until
//! the parser host wires them together.

/// Documented public surface of the original module (exported names).
pub const EXPORTED_SURFACE: &[&str] = &[
    "Parser",
    "Action",
    "Color",
    "CursorAction",
    "CursorDirection",
];
