//! Rust exports derived from original: src/hooks/useDynamicConfig.ts
//!
//! Dynamic config hook: returns the default value immediately and swaps in
//! the fetched value once the config resolves. In test environments the
//! fetch is skipped to prevent hangs — the decision below is the full test
//! semantics; the fetch itself rides the analytics port.

/// Whether the initial fetch should run in this environment.
pub fn should_fetch(env: Option<&str>) -> bool {
    env != Some("test")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_fetch_in_test_env() {
        assert!(!should_fetch(Some("test")));
        assert!(should_fetch(None));
        assert!(should_fetch(Some("production")));
    }
}
