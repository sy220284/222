//! Rust exports derived from original: src/ink/components/NoSelect.tsx
//!
//! Selection exclusion decision core: cells inside a NoSelect box are
//! skipped by both the selection highlight and copied text. `fromLeftEdge`
//! extends the exclusion zone from column 0 to the box's right edge for
//! every row it occupies — used for gutters (line numbers, diff sigils)
//! inside indented containers so multi-row drags stay clean. Cell-level
//! geometry belongs to the ink runtime.

/// Effective exclusion extent for a NoSelect box at `left_offset` columns.
pub fn exclusion_left(from_left_edge: bool, left_offset: usize) -> usize {
    if from_left_edge {
        0
    } else {
        left_offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_and_offset_semantics() {
        assert_eq!(exclusion_left(true, 4), 0);
        assert_eq!(exclusion_left(false, 4), 4);
    }
}
