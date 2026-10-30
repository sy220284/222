//! Rust exports derived from original: src/cli/transports/transportUtils.ts
//!
//! Full equivalence: transport selection priority — CCR_V2 env selects SSE
//! reads + POST writes, POST_FOR_SESSION_INGRESS_V2 selects Hybrid (WS reads
//! + POST writes), default is WebSocket. The concrete transports are
//! separate files; this is the decision core.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportKind {
    /// SSE reads + POST writes (v2 session ingress).
    Sse,
    /// WS reads + POST writes.
    Hybrid,
    /// WS reads + WS writes (default).
    WebSocket,
}

/// `getTransportForUrl` decision core.
pub fn transport_for_url(use_ccr_v2: bool, post_for_session_ingress_v2: bool) -> TransportKind {
    if use_ccr_v2 {
        TransportKind::Sse
    } else if post_for_session_ingress_v2 {
        TransportKind::Hybrid
    } else {
        TransportKind::WebSocket
    }
}

/// v2 SSE stream URL derivation: session URL + `/worker/events/stream`.
pub fn sse_stream_url(session_url: &str) -> String {
    format!("{}/worker/events/stream", session_url.trim_end_matches('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_priority() {
        assert_eq!(transport_for_url(true, true), TransportKind::Sse);
        assert_eq!(transport_for_url(false, true), TransportKind::Hybrid);
        assert_eq!(transport_for_url(false, false), TransportKind::WebSocket);
    }

    #[test]
    fn stream_url_appends_path() {
        assert_eq!(
            sse_stream_url("https://x/sessions/abc"),
            "https://x/sessions/abc/worker/events/stream"
        );
    }
}
