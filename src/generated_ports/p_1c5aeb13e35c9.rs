//! Rust exports derived from original: src/migrations/migrateFennecToOpus.ts
//!
//! Full equivalence of the alias remap table (ant-gated in the original):
//! fennec-latest → opus; fennec-latest] → opus]; fennec-fast-latest → opus]
//! + fast mode; opus-4-5-fast → opus + fast mode. Only userSettings is
//! touched — rewriting the same source keeps it idempotent without a
//! completion flag, and project/local/policy aliases stay alone.

/// Remap outcome: new alias plus whether fast mode turns on.
pub fn remap_fennec_alias(saved: &str) -> Option<(&'static str, bool)> {
    match saved {
        "fennec-latest" => Some(("opus", false)),
        "fennec-latest]" => Some(("opus]", false)),
        "fennec-fast-latest" => Some(("opus]", true)),
        "opus-4-5-fast" => Some(("opus", true)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_matches_original() {
        assert_eq!(remap_fennec_alias("fennec-latest"), Some(("opus", false)));
        assert_eq!(remap_fennec_alias("fennec-latest]"), Some(("opus]", false)));
        assert_eq!(
            remap_fennec_alias("fennec-fast-latest"),
            Some(("opus]", true))
        );
        assert_eq!(remap_fennec_alias("opus-4-5-fast"), Some(("opus", true)));
        assert_eq!(remap_fennec_alias("opus"), None);
        assert_eq!(remap_fennec_alias("sonnet"), None);
    }
}
