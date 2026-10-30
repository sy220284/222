//! Rust exports derived from original: src/utils/undercover.ts
//!
//! External-build semantics (USER_TYPE is a build-time define and the
//! ant-only branches are dead-code eliminated): every function reduces to a
//! trivial return, so `is_undercover()` is constant false in this build and
//! the attribution strip never runs. The ant-internal repo allowlist is
//! vendor telemetry/attribution surface (excluded).

/// `isUndercover` — external build reduces to false.
pub fn is_undercover() -> bool {
    false
}

/// `getUndercoverInstructions` — external build reduces to empty.
pub fn get_undercover_instructions() -> Vec<String> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_build_trivial() {
        assert!(!is_undercover());
        assert!(get_undercover_instructions().is_empty());
    }
}
