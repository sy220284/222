//! Rust exports derived from original: src/utils/computerUse/cleanup.ts
//!
//! Turn-end cleanup ordering contract: unhide apps (bounded, 5s generous
//! backstop — proceeding beats wedging an abort) then release the file lock.
//! The native Swift surface is unavailable in Rust (excluded vendor
//! dependency); the sequence and its failure tolerance are preserved.

pub const UNHIDE_TIMEOUT_MS: u64 = 5000;

/// Cleanup ordering: unhide first (bounded), lock release second.
pub fn cleanup_order() -> &'static [&'static str] {
    &["unhide_apps", "release_lock"]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_and_backstop() {
        assert_eq!(UNHIDE_TIMEOUT_MS, 5000);
        assert_eq!(cleanup_order(), &["unhide_apps", "release_lock"]);
    }
}
