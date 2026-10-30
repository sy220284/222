//! Transport family for SDK/remote ingress. 2.1.88 reference:
//! `src/cli/transports/{transportUtils,SSETransport,WebSocketTransport,
//! HybridTransport,SerialBatchEventUploader,WorkerStateUploader,ccrClient}.ts`.
//!
//! Selection priority (source `getTransportForUrl`):
//! 1. SSE (SSE reads + POST writes) when `ATLAS_USE_CCR_V2` is truthy —
//!    the session URL gains `/worker/events/stream` after ws→http conversion
//! 2. Hybrid (WS reads + POST writes) when
//!    `ATLAS_POST_FOR_SESSION_INGRESS_V2` is truthy
//! 3. WebSocket (WS reads + WS writes) — default for ws:// URLs

pub mod ccr_client;
pub mod hybrid;
pub mod serial_batch_event_uploader;
pub mod sse;
pub mod web_socket;
pub mod worker_state_uploader;

pub use ccr_client::{
    accumulate_stream_events, clear_stream_accumulator_for_message, create_stream_accumulator,
    CcrClient, StreamAccumulatorState,
};
pub use serial_batch_event_uploader::{
    SerialBatchEventUploader, UploaderConfig, UploaderError, UploaderSend,
};
pub use sse::{convert_sse_url_to_post_url, parse_sse_frames, SseTransport};
pub use web_socket::WebSocketTransport;
pub use worker_state_uploader::{coalesce_patches, WorkerStateUploader};

use anyhow::Result;
use serde_json::Value;

/// The transport flavour chosen for a URL (source `getTransportForUrl`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportKind {
    Sse,
    Hybrid,
    WebSocket,
}

fn is_env_truthy(name: &str) -> bool {
    matches!(
        std::env::var(name).ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("on")
    )
}

/// Derive the SSE stream URL for a session URL (ws(s) → http(s), path gains
/// `/worker/events/stream`).
pub fn sse_stream_url_for_session(session_url: &str) -> Result<String> {
    let mut url = reqwest::Url::parse(session_url)?;
    match url.scheme() {
        "ws" => {
            url.set_scheme("http")
                .map_err(|_| anyhow::anyhow!("Invalid SSE URL scheme"))?;
        }
        "wss" => {
            url.set_scheme("https")
                .map_err(|_| anyhow::anyhow!("Invalid SSE URL scheme"))?;
        }
        "http" | "https" => {}
        other => anyhow::bail!("Unsupported SSE source URL scheme: {other}"),
    }
    // URL.pathname in the original transport preserves query parameters.
    // String concatenation would append /worker/events/stream *after* ?token.
    let stream_path = format!("{}/worker/events/stream", url.path().trim_end_matches('/'));
    url.set_path(&stream_path);
    Ok(url.to_string())
}

/// Resolve which transport a session URL should use (source
/// `getTransportForUrl`; ws→SSE conversion preserved).
pub fn get_transport_for_url(url: &str) -> Result<TransportKind> {
    select_transport(
        url,
        is_env_truthy("ATLAS_USE_CCR_V2"),
        is_env_truthy("ATLAS_POST_FOR_SESSION_INGRESS_V2"),
    )
}

/// Pure mode selection enables exhaustive tests without mutating process env.
/// A remote SDK session then constructs and runs the matching live transport.
fn select_transport(url: &str, use_ccr_v2: bool, post_for_ingress: bool) -> Result<TransportKind> {
    let parsed = reqwest::Url::parse(url)?;
    match parsed.scheme() {
        "ws" | "wss" | "http" | "https" if use_ccr_v2 => Ok(TransportKind::Sse),
        "ws" | "wss" if post_for_ingress => Ok(TransportKind::Hybrid),
        "ws" | "wss" => Ok(TransportKind::WebSocket),
        other => anyhow::bail!("Unsupported protocol for session URL: {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sse_url_derivation_preserves_auth_query_and_ipv6_authority() {
        assert_eq!(
            sse_stream_url_for_session("wss://example.com/sessions/s1?token=a%2Fb&mode=1").unwrap(),
            "https://example.com/sessions/s1/worker/events/stream?token=a%2Fb&mode=1"
        );
        assert_eq!(
            sse_stream_url_for_session("ws://[::1]:9912/s1/?nonce=7").unwrap(),
            "http://[::1]:9912/s1/worker/events/stream?nonce=7"
        );
        assert!(sse_stream_url_for_session("file:///tmp/sessions/s1").is_err());
    }

    #[test]
    fn sse_stream_url_conversion() {
        assert_eq!(
            sse_stream_url_for_session("wss://api.example.com/v2/sessions/s1").unwrap(),
            "https://api.example.com/v2/sessions/s1/worker/events/stream"
        );
        assert_eq!(
            sse_stream_url_for_session("http://127.0.0.1:9915/sessions/s1/").unwrap(),
            "http://127.0.0.1:9915/sessions/s1/worker/events/stream"
        );
    }

    #[test]
    fn source_mode_precedence_and_protocol_errors() {
        assert_eq!(
            select_transport("ws://localhost/s", false, false).unwrap(),
            TransportKind::WebSocket
        );
        assert_eq!(
            select_transport("wss://localhost/s", false, true).unwrap(),
            TransportKind::Hybrid
        );
        assert_eq!(
            select_transport("wss://localhost/s", true, true).unwrap(),
            TransportKind::Sse
        );
        assert_eq!(
            select_transport("https://localhost/s", true, false).unwrap(),
            TransportKind::Sse
        );
        assert!(select_transport("https://localhost/s", false, false).is_err());
        assert!(select_transport("ftp://localhost/s", true, true).is_err());
        assert!(select_transport("not a url", true, true).is_err());
    }

    #[test]
    fn unsupported_protocol_rejected() {
        assert!(get_transport_for_url("ftp://host/x").is_err());
    }
}
