//! Rust exports derived from original: src/ink/layout/engine.ts
//!
//! `createLayoutNode` delegates to a Yoga-backed layout node. This crate has
//! no Yoga binding yet, so the factory records the contract and the ink
//! runtime host supplies the concrete layout engine.

/// `createLayoutNode` — returns a fresh layout node root.
pub fn create_layout_node() -> LayoutNode {
    LayoutNode
}

/// Placeholder node type until the layout engine port lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutNode;
