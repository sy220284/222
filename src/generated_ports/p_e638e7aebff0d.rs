//! Rust exports derived from original: src/ink/instances.ts
//!
//! All Ink instances live in a map keyed by the output stream so consecutive
//! render() calls reuse the same instance; instance.js removes itself on
//! unmount. The map lives in its own file in the original to break the
//! render/instance cycle — the Rust ink host owns the equivalent registry.

/// Semantics marker: keyed by output stream identity.
pub const INSTANCES_MAP_DOC: &str = "Map<WriteStream, Ink> reused across render() calls";
