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
    if is_env_truthy("ATLAS_USE_CCR_V2") {
        // CCR v2: the session URL becomes an SSE stream URL. Any ws:// or
        // ws(s) session URL is accepted after scheme conversion.
        return Ok(TransportKind::Sse);
    }
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("ws://") || lower.starts_with("wss://") {
        if is_env_truthy("ATLAS_POST_FOR_SESSION_INGRESS_V2") {
            return Ok(TransportKind::Hybrid);
        }
        return Ok(TransportKind::WebSocket);
    }
    anyhow::bail!("Unsupported protocol for session URL: {url}")
}

/// One outbound sink shared by SDK mode: forwards newline-JSON messages to
/// the chosen transport when configured, otherwise stdout ndjson (default).
pub enum SdkOutbound {
    Stdout,
    Sse(crate::transports::sse::SseTransport, String),
}

impl SdkOutbound {
    /// Build from a session URL and env flags; None falls back to stdout.
    pub fn resolve(transport_url: Option<&str>) -> Result<Self> {
        let Some(url) = transport_url else {
            return Ok(Self::Stdout);
        };
        match get_transport_for_url(url)? {
            TransportKind::Sse => {
                let sse_url = sse_stream_url_for_session(url)?;
                Ok(Self::Sse(
                    crate::transports::sse::SseTransport::new(
                        sse_url.clone(),
                        Vec::new(),
                        None,
                        Arc::new(|| Vec::new()),
                    ),
                    crate::transports::sse::convert_sse_url_to_post_url(&sse_url)?,
                ))
            }
            TransportKind::Hybrid | TransportKind::WebSocket => {
                // WS-family ingress writes POST frames through the hybrid
                // outbound path; reads arrive on the WS data channel.
                let sse_url = sse_stream_url_for_session(url)?;
                Ok(Self::Sse(
                    crate::transports::sse::SseTransport::new(
                        sse_url.clone(),
                        Vec::new(),
                        None,
                        Arc::new(|| Vec::new()),
                    ),
                    crate::transports::sse::convert_sse_url_to_post_url(&sse_url)?,
                ))
            }
        }
    }

    /// Emit one stdout message: stdout always (SDK protocol), plus a POST to
    /// the configured endpoint when a transport is active.
    pub async fn emit(&self, message: &Value) -> Result<()> {
        print!("{}", crate::ndjson::stringify_value(message));
        use std::io::Write;
        let _ = std::io::stdout().flush();
        if let Self::Sse(_transport, post_url) = self {
            let client = reqwest::Client::new();
            client
                .post(post_url)
                .header("Content-Type", "application/json")
                .body(crate::ndjson::stringify_value(message))
                .send()
                .await?
                .error_for_status()?;
        }
        Ok(())
    }
}

use std::sync::Arc;

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
    fn unsupported_protocol_rejected() {
        assert!(get_transport_for_url("ftp://host/x").is_err());
    }
}
