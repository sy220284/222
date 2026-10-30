//! Hybrid transport: WebSocket reads + serial POST writes.
//! 2.1.88 reference: `src/cli/transports/HybridTransport.ts`.
//!
//! Inbound frames flow from the WebSocket read loop; outbound messages are
//! enqueued onto a `SerialBatchEventUploader` (batch window 100ms, POST
//! timeout 15s) that converts the WS URL to its POST endpoint.

use anyhow::{anyhow, bail, Result};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;

use super::serial_batch_event_uploader::{UploaderConfig, UploaderError, UploaderSend};
use super::web_socket::{parse_ws_url, MinimalWsClient, WebSocketTransport};

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
    let (path_part, query) = tail.split_once('?').map_or((tail, ""), |(p, q)| (p, q));
    let mut path = path_part.replacen("/ws/", "/session/", 1);
    if !path.ends_with("/events") {
        if path.ends_with('/') {
            path.push_str("events");
        } else {
            path.push_str("/events");
        }
    }
    let suffix = if query.is_empty() { String::new() } else { format!("?{query}") };
    Ok(format!("{scheme}//{authority}{path}{suffix}"))
}

pub type HybridAuth = Arc<dyn Fn() -> Vec<(String, String)> + Send + Sync>;

/// Outbound writer: serial batch uploads over POST while reads stay on WS.
pub struct HybridOutbound {
    uploader: Arc<super::serial_batch_event_uploader::SerialBatchEventUploader<Value>>,
    post_url: String,
    auth: HybridAuth,
    stream_event_buffer: Arc<Mutex<Vec<Value>>>,
    timer_running: Arc<AtomicBool>,
}

impl HybridOutbound {
    pub fn new(ws_url: &str, auth: HybridAuth) -> Result<Self> {
        let post_url = convert_ws_url_to_post_url(ws_url)?;
        let sender_auth = auth.clone();
        let sender_url = post_url.clone();
        let send: UploaderSend<Value> = Arc::new(move |batch| {
            let post_url = sender_url.clone();
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
            uploader: Arc::new(super::serial_batch_event_uploader::SerialBatchEventUploader::new(config)),
            post_url,
            auth,
            stream_event_buffer: Arc::new(Mutex::new(Vec::new())),
            timer_running: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn post_url(&self) -> &str {
        &self.post_url
    }

    /// Stream deltas coalesce over 100 ms. For all other messages, wait until
    /// queued POSTs are persisted, mirroring HybridTransport.write in 2.1.88.
    pub async fn write(&self, message: &Value) {
        if message.get("type").and_then(Value::as_str) == Some("stream_event") {
            self.stream_event_buffer.lock().expect("hybrid stream buffer").push(message.clone());
            if !self.timer_running.swap(true, Ordering::SeqCst) {
                let buffer = Arc::clone(&self.stream_event_buffer);
                let running = Arc::clone(&self.timer_running);
                let uploader = Arc::clone(&self.uploader);
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(BATCH_FLUSH_INTERVAL_MS)).await;
                    let batch = std::mem::take(&mut *buffer.lock().expect("hybrid stream buffer"));
                    running.store(false, Ordering::SeqCst);
                    if !batch.is_empty() {
                        uploader.enqueue(batch).await;
                    }
                });
            }
            return;
        }
        let mut events = std::mem::take(&mut *self.stream_event_buffer.lock().expect("hybrid stream buffer"));
        events.push(message.clone());
        self.uploader.enqueue(events).await;
        self.uploader.flush().await;
    }

    pub async fn flush(&self) {
        let events = std::mem::take(&mut *self.stream_event_buffer.lock().expect("hybrid stream buffer"));
        if !events.is_empty() {
            self.uploader.enqueue(events).await;
        }
        self.uploader.flush().await;
    }

    pub fn close(&self) {
        self.stream_event_buffer.lock().expect("hybrid stream buffer").clear();
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
    let mut conn = MinimalWsClient::connect(
        &host, port, &path, &headers, ws_url.starts_with("wss://")
    ).await?;
    loop {
        match conn.read_message().await {
            Ok(super::web_socket::WsIncoming::Text(text)) => {
                let _ = data_tx.send(text);
            }
            Ok(super::web_socket::WsIncoming::Close(_)) | Err(_) => bail!("hybrid inbound closed"),
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
        outbound.write(&serde_json::json!({"type":"stream_event"})).await;
        // A stream event returns before the short batching window expires.
        outbound.close();
    }
}
