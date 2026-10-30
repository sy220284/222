//! Rust exports derived from original: src/utils/apiPreconnect.ts
//!
//! Full equivalence at the decision layer: preconnect fires exactly once and
//! only when no proxy/mTLS/unix-socket transport is configured and the
//! backend is the first-party API (Bedrock/Vertex/Foundry use different
//! endpoints and auth). Connection warming itself rides the reqwest global
//! pool, mirroring Bun's shared keep-alive pool.

/// Whether the fire-and-forget preconnect should kick off.
pub fn preconnect_should_fire(
    already_fired: bool,
    proxy_configured: bool,
    mtls_configured: bool,
    unix_socket_configured: bool,
    backend: &str,
) -> bool {
    !already_fired
        && !proxy_configured
        && !mtls_configured
        && !unix_socket_configured
        && backend == "api"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gates_match_original() {
        assert!(preconnect_should_fire(false, false, false, false, "api"));
        assert!(!preconnect_should_fire(true, false, false, false, "api"));
        assert!(!preconnect_should_fire(false, true, false, false, "api"));
        assert!(!preconnect_should_fire(false, false, true, false, "api"));
        assert!(!preconnect_should_fire(
            false, false, false, true, "bedrock"
        ));
        assert!(!preconnect_should_fire(
            false, false, false, false, "vertex"
        ));
    }
}
