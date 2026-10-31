//! Rust exports derived from original: src/services/internalLogging.ts
//!
//! External-build semantics: the ant gate constant-folds to false, so the
//! K8s namespace probe never runs and returns null. Internal-cluster
//! logging destinations are vendor telemetry surface (excluded).

/// `getKubernetesNamespace` — external build reduces to null.
pub fn get_kubernetes_namespace() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_build_null() {
        assert!(get_kubernetes_namespace().is_none());
    }
}
