//! Rust exports derived from original: src/ink/components/CursorDeclarationContext.ts
//!
//! Cursor declaration contract: (relativeX, relativeY, node). The setter's
//! optional clearIfNode makes null a conditional clear — only cleared when
//! the currently-declared node matches, so sibling components transferring
//! focus cannot clobber each other regardless of layout-effect order.

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CursorDeclaration {
    pub relative_x: i64,
    pub relative_y: i64,
    pub node_id: u64,
}

/// Conditional clear semantics for the setter.
pub fn apply_clear(
    current: Option<CursorDeclaration>,
    clear_if_node: Option<u64>,
) -> Option<CursorDeclaration> {
    match (current, clear_if_node) {
        // Owner clears itself.
        (Some(cur), Some(node)) if cur.node_id == node => None,
        // Unconditional clear.
        (Some(_), None) => None,
        // A sibling's clear does not clobber the owner's declaration.
        (Some(cur), Some(_)) => Some(cur),
        (None, _) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decl(node: u64) -> CursorDeclaration {
        CursorDeclaration {
            relative_x: 1,
            relative_y: 2,
            node_id: node,
        }
    }

    #[test]
    fn sibling_clear_does_not_clobber() {
        // Sibling (node 2) clears while node 1 owns the declaration: kept.
        assert_eq!(apply_clear(Some(decl(1)), Some(2)), Some(decl(1)));
        // Owner clears itself: gone.
        assert_eq!(apply_clear(Some(decl(1)), Some(1)), None);
        // Unconditional clear.
        assert_eq!(apply_clear(Some(decl(1)), None), None);
    }
}
