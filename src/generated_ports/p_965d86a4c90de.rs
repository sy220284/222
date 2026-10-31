//! Rust exports derived from original: src/ink/node-cache.ts
//!
//! Cached layout bounds per rendered node (blit + clearing): `top` stores
//! the yoga-local computed top so ScrollBox viewport culling can skip yoga
//! reads for clean children (O(dirty) instead of O(mounted) first-pass).
//! Pending clears mark removed-children rects; an absolute-positioned
//! removal disables blit for the next frame (cross-subtree paints) and the
//! flag resets at render start. WeakMap identity maps belong to the ink
//! DOM host; the value shapes and the blit-disable flag are the port.

/// `CachedLayout` — per-node bounds for blit + clearing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CachedLayout {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    /// yoga-local computed top for viewport culling.
    pub top: Option<u16>,
}

/// Whether any pending clear came from an absolute-positioned node.
#[derive(Default)]
pub struct NodeCacheFlags {
    pub absolute_pending_clear: bool,
}
