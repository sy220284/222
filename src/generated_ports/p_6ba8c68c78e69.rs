//! Rust exports derived from original: src/components/ui/OrderedListItem.tsx
//!
//! Ordered list item: renders its marker dim-colored from the surrounding
//! list context, then children. The ordered-list context (marker numbering)
//! is owned by the ink runtime host.

/// Marker text of this item (context-injected in the original).
pub fn item_marker(number: u32, separator: &str) -> String {
    format!("{number}{separator}")
}
