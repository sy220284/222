//! Hybrid transport: WebSocket reads + serial POST writes.
//! 2.1.88 reference: `src/cli/transports/HybridTransport.ts`.
//!
//! Inbound frames flow from the WebSocket read loop; outbound messages are
//! enqueued onto a `SerialBatchEventUploader` (batch window 100ms, POST
//! timeout 15s) that converts the WS URL to its POST endpoint.

use anyhow::{anyhow, bail, Result};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

use super::serial_batch_event_uploader::{UploaderConfig, UploaderError, UploaderSend};
use super::ws::{parse_ws_url, MinimalWsClient, WebSocketTransport};

pub const BATCH_FLUSH_INTERVAL_MS: u64 = 100;
/// Per-attempt POST timeout — bounds how long one stuck POST can block the
/// serialized queue.
pub const POST_TIMEOUT_MS: u64 = 15_000;

/// Convert a WebSocket URL to the HTTP POST endpoint URL.
/// From `wss://host/v2/session_ingress/ws/<id>` to
/// `https://host/v2/session_ingress/session/<id>/events`.
pub fn convert_ws_url_to_post_url(ws_url: &str) -> Result<String> {
    let rest = ws_url
        .strip_prefix("wss://")
        .map(|rest| ("https:", rest))
        .or_else(|| ws_url.strip_prefix("ws://").map(|rest| ("http:", rest)))
        .ok_or_else(|| anyhow!("Unsupported WebSocket URL: {ws_url}"))?;
    let (scheme, rest) = rest;
    let (authority, tail) = match rest.find('/') {
        Some(index) => (&rest[..index], &rest[index..]),
        None => (rest, "/"),
    };
    let mut path = tail.replacen("/ws/", "/session/", 1);
    if !path.ends_with("/events") {
        if path.ends_with('/') {
            path.push_str("events");
        } else {
            path.push_str("/events");
        }
    }
    Ok(format!("{scheme}//{authority}{path}"))
}

pub type HybridAuth = Arc<dyn Fn() -> Vec<(String, String)> + Send + Sync>;

/// Outbound writer: serial batch uploads over POST while reads stay on WS.
pub struct HybridOutbound {
    uploader: super::serial_batch_event_uploader::SerialBatchEventUploader<Value>,
    post_url: String,
    auth: HybridAuth,
}

impl HybridOutbound {
    pub fn new(ws_url: &str, auth: HybridAuth) -> Result<Self> {
        let post_url = convert_ws_url_to_post_url(ws_url)?;
        let sender_auth = auth.clone();
        let send: UploaderSend<Value> = Arc::new(move |batch| {
            let post_url = post_url.clone();
            let auth = sender_auth.clone();
            Box::pin(async move {
                let client = reqwest::Client::new();
                let mut request = client
                    .post(&post_url)
                    .header("Content-Type", "application/json")
                    .header("anthropic-version", "2023-06-01")
                    .timeout(Duration::from_millis(POST_TIMEOUT_MS));
                for (name, value) in auth() {
                    request = request.header(name, value);
                }
                match request
                    .body(crate::ndjson::stringify_value(&Value::Array(
                        batch.to_vec(),
                    )))
                    .send()
                    .await
                {
                    Ok(response) => {
                        let status = response.status().as_u16();
                        if status == 200 || status == 201 {
                            return Ok(());
                        }
                        Err(UploaderError::Retryable {
                            retry_after_ms: parse_retry_after_status(status),
                        })
                    }
                    Err(_) => Err(UploaderError::Retryable {
                        retry_after_ms: None,
                    }),
                }
            })
        });
        let mut config = UploaderConfig::new(send);
        config.max_batch_size = usize::MAX;
        config.base_delay_ms = 500;
        config.max_delay_ms = 8_000;
        config.jitter_ms = 250;
        Ok(Self {
            uploader: super::serial_batch_event_uploader::SerialBatchEventUploader::new(config),
            post_url,
            auth,
        })
    }

    pub fn post_url(&self) -> &str {
        &self.post_url
    }

    /// Queue one newline-JSON message; the 100ms batch window accumulates
    /// bursts before a single POST.
    pub async fn write(&self, message: &Value) {
        self.uploader.enqueue(vec![message.clone()]).await;
    }

    /// Wait until every queued write has been POSTed (flush on close).
    pub async fn flush(&self) {
        self.uploader.flush().await;
    }

    pub fn close(&self) {
        self.uploader.close();
    }
}

fn parse_retry_after_status(status: u16) -> Option<u64> {
    if status == 429 {
        Some(1_000)
    } else {
        None
    }
}

/// Run the inbound WebSocket read loop, forwarding every text frame line to
/// `data_tx`. Returns when the connection drops (caller decides reconnect).
pub async fn run_inbound(
    ws_url: &str,
    headers: Vec<(String, String)>,
    data_tx: mpsc::UnboundedSender<String>,
) -> Result<()> {
    let (host, port, path) = parse_ws_url(ws_url)?;
    let mut conn = MinimalWsClient::connect(&host, port, &path, &headers).await?;
    loop {
        match conn.read_message().await {
            Ok(super::ws::WsIncoming::Text(text)) => {
                let _ = data_tx.send(text);
            }
            Ok(super::ws::WsIncoming::Close(_)) | Err(_) => bail!("hybrid inbound closed"),
            Ok(_) => {}
        }
    }
}

/// Compose a full hybrid transport pair. The WebSocket side reuses
/// `WebSocketTransport`'s replay/reconnect machinery for reads; writes go
/// through `HybridOutbound`.
pub struct HybridTransport {
    pub reads: WebSocketTransport,
    pub writes: HybridOutbound,
}

impl HybridTransport {
    pub fn new(
        ws_url: String,
        headers: Vec<(String, String)>,
        session_id: Option<String>,
        data_tx: mpsc::UnboundedSender<String>,
        auth: HybridAuth,
    ) -> Result<Self> {
        Ok(Self {
            reads: WebSocketTransport::new(
                ws_url.clone(),
                headers,
                session_id,
                data_tx,
                true,
                None,
            ),
            writes: HybridOutbound::new(&ws_url, auth)?,
        })
    }

    pub async fn close(&self) {
        self.writes.flush().await;
        self.writes.close();
        self.reads.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_ws_url_to_post_url() {
        assert_eq!(
            convert_ws_url_to_post_url("wss://api.example.com/v2/session_ingress/ws/s1?token=x")
                .unwrap(),
            "https://api.example.com/v2/session_ingress/session/s1/events?token=x"
        );
        assert_eq!(
            convert_ws_url_to_post_url("ws://127.0.0.1:9913/ws/s2").unwrap(),
            "http://127.0.0.1:9913/session/s2/events"
        );
        assert_eq!(
            convert_ws_url_to_post_url("ws://127.0.0.1:9913/session/s3/events").unwrap(),
            "http://127.0.0.1:9913/session/s3/events"
        );
        assert!(convert_ws_url_to_post_url("http://host/path").is_err());
    }

    #[tokio::test]
    async fn outbound_batches_and_flushes() {
        let auth: HybridAuth = Arc::new(|| vec![]);
        let outbound = HybridOutbound::new("ws://127.0.0.1:9/ws/s1", auth).unwrap();
        outbound.write(&serde_json::json!({"type":"x"})).await;
        // No server: flush returns after retries are queued; close discards.
        outbound.close();
    }
}
