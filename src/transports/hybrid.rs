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
/// Source close() gives queued uploads a bounded three-second grace period.
pub const CLOSE_GRACE_MS: u64 = 3_000;

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
    let suffix = if query.is_empty() {
        String::new()
    } else {
        format!("?{query}")
    };
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
    sequence: Arc<tokio::sync::Mutex<()>>,
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
                    .timeout(Duration::from_millis(POST_TIMEOUT_MS));
                for (name, value) in auth() {
                    request = request.header(name, value);
                }
                match request
                    .body(crate::ndjson::stringify_value(&serde_json::json!({
                        "events": batch
                    })))
                    .send()
                    .await
                {
                    Ok(response) => {
                        let status = response.status().as_u16();
                        if response.status().is_success() {
                            return Ok(());
                        }
                        // Upstream treats permanent client errors as delivered
                        // (discard) rather than retrying the same rejected data.
                        if (400..500).contains(&status) && status != 429 {
                            return Ok(());
                        }
                        Err(UploaderError::Retryable {
                            retry_after_ms: None,
                        })
                    }
                    Err(_) => Err(UploaderError::Retryable {
                        retry_after_ms: None,
                    }),
                }
            })
        });
        let mut config = UploaderConfig::new(send);
        // 2.1.88 HybridTransport caps every POST at 500 messages and
        // randomizes retries by up to 1,000 ms.
        config.max_batch_size = 500;
        config.base_delay_ms = 500;
        config.max_delay_ms = 8_000;
        config.jitter_ms = 1_000;
        Ok(Self {
            uploader: Arc::new(
                super::serial_batch_event_uploader::SerialBatchEventUploader::new(config),
            ),
            post_url,
            auth,
            stream_event_buffer: Arc::new(Mutex::new(Vec::new())),
            timer_running: Arc::new(AtomicBool::new(false)),
            sequence: Arc::new(tokio::sync::Mutex::new(())),
        })
    }

    pub fn post_url(&self) -> &str {
        &self.post_url
    }

    /// Stream deltas coalesce over 100 ms. For all other messages, wait until
    /// queued POSTs are persisted, mirroring HybridTransport.write in 2.1.88.
    pub async fn write(&self, message: &Value) {
        if message.get("type").and_then(Value::as_str) == Some("stream_event") {
            self.stream_event_buffer
                .lock()
                .expect("hybrid stream buffer")
                .push(message.clone());
            if !self.timer_running.swap(true, Ordering::SeqCst) {
                let buffer = Arc::clone(&self.stream_event_buffer);
                let running = Arc::clone(&self.timer_running);
                let uploader = Arc::clone(&self.uploader);
                let sequence = Arc::clone(&self.sequence);
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(BATCH_FLUSH_INTERVAL_MS)).await;
                    // Timer and immediate writes must enqueue in the same order.
                    let _sequence = sequence.lock().await;
                    let batch = std::mem::take(&mut *buffer.lock().expect("hybrid stream buffer"));
                    running.store(false, Ordering::SeqCst);
                    if !batch.is_empty() {
                        uploader.enqueue(batch).await;
                    }
                });
            }
            return;
        }
        let _sequence = self.sequence.lock().await;
        let mut events = std::mem::take(
            &mut *self
                .stream_event_buffer
                .lock()
                .expect("hybrid stream buffer"),
        );
        events.push(message.clone());
        self.uploader.enqueue(events).await;
        self.uploader.flush().await;
    }

    pub async fn flush(&self) {
        let _sequence = self.sequence.lock().await;
        let events = std::mem::take(
            &mut *self
                .stream_event_buffer
                .lock()
                .expect("hybrid stream buffer"),
        );
        if !events.is_empty() {
            self.uploader.enqueue(events).await;
        }
        self.uploader.flush().await;
    }

    pub fn close(&self) {
        self.stream_event_buffer
            .lock()
            .expect("hybrid stream buffer")
            .clear();
        self.uploader.close();
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
    let mut conn =
        MinimalWsClient::connect(&host, port, &path, &headers, ws_url.starts_with("wss://"))
            .await?;
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
                Some(Arc::clone(&auth)),
            ),
            writes: HybridOutbound::new(&ws_url, auth)?,
        })
    }

    pub async fn close(&self) {
        self.reads.close();
        let _ =
            tokio::time::timeout(Duration::from_millis(CLOSE_GRACE_MS), self.writes.flush()).await;
        self.writes.close();
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
    async fn post_events_envelope_and_permanent_http_errors_match_source() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;
        for status in [204_u16, 400_u16] {
            let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
            let port = listener.local_addr().unwrap().port();
            let server = tokio::spawn(async move {
                let (mut peer, _) = listener.accept().await.unwrap();
                let mut buffer = Vec::new();
                let mut chunk = [0_u8; 1024];
                let end = loop {
                    if let Some(i) = buffer.windows(4).position(|b| b == b"\r\n\r\n") {
                        break i + 4;
                    }
                    let n = peer.read(&mut chunk).await.unwrap();
                    assert!(n > 0 && buffer.len() < 16384);
                    buffer.extend_from_slice(&chunk[..n]);
                };
                let header = String::from_utf8(buffer[..end].to_vec()).unwrap();
                assert!(header.starts_with("POST /session/s1/events HTTP/1.1"));
                let content_length: usize = header
                    .lines()
                    .find_map(|line| {
                        line.split_once(':').and_then(|(name, value)| {
                            name.eq_ignore_ascii_case("Content-Length")
                                .then(|| value.trim().parse().unwrap())
                        })
                    })
                    .unwrap();
                while buffer.len() - end < content_length {
                    let n = peer.read(&mut chunk).await.unwrap();
                    assert!(n > 0);
                    buffer.extend_from_slice(&chunk[..n]);
                }
                let body: Value =
                    serde_json::from_slice(&buffer[end..end + content_length]).unwrap();
                let reply = format!(
                    "HTTP/1.1 {status} status\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
                peer.write_all(reply.as_bytes()).await.unwrap();
                body
            });
            let outbound =
                HybridOutbound::new(&format!("ws://127.0.0.1:{port}/ws/s1"), Arc::new(Vec::new))
                    .unwrap();
            tokio::time::timeout(
                Duration::from_secs(3),
                outbound.write(&serde_json::json!({"type":"result","uuid":"r1"})),
            )
            .await
            .expect("both successful and permanent response must drain");
            let actual = server.await.unwrap();
            assert_eq!(
                actual,
                serde_json::json!({
                    "events": [{"type":"result","uuid":"r1"}]
                })
            );
            outbound.close();
        }
    }

    #[tokio::test]
    async fn stream_events_precede_immediate_result_in_same_http_batch() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = tokio::spawn(async move {
            let (mut peer, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let mut chunk = [0_u8; 1024];
            let end = loop {
                if let Some(i) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
                    break i + 4;
                }
                let n = peer.read(&mut chunk).await.unwrap();
                assert!(n > 0 && bytes.len() < 16384);
                bytes.extend_from_slice(&chunk[..n]);
            };
            let headers = String::from_utf8_lossy(&bytes[..end]);
            let len: usize = headers
                .lines()
                .find_map(|line| {
                    line.split_once(':').and_then(|(name, value)| {
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse().unwrap())
                    })
                })
                .unwrap();
            while bytes.len() - end < len {
                let n = peer.read(&mut chunk).await.unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&chunk[..n]);
            }
            let payload: Value = serde_json::from_slice(&bytes[end..end + len]).unwrap();
            peer.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                .await
                .unwrap();
            payload
        });
        let outbound =
            HybridOutbound::new(&format!("ws://127.0.0.1:{port}/ws/s1"), Arc::new(Vec::new))
                .unwrap();
        outbound
            .write(&serde_json::json!({"type":"stream_event","uuid":"first"}))
            .await;
        tokio::time::timeout(
            Duration::from_secs(4),
            outbound.write(&serde_json::json!({"type":"result","uuid":"second"})),
        )
        .await
        .unwrap();
        let received = server.await.unwrap();
        assert_eq!(received["events"][0]["uuid"], "first");
        assert_eq!(received["events"][1]["uuid"], "second");
        outbound.close();
    }

    #[tokio::test]
    async fn hybrid_websocket_read_uses_dynamic_authorization_header() {
        use tokio::io::AsyncReadExt;
        use tokio::net::TcpListener;
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = tokio::spawn(async move {
            let (mut peer, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut chunk = [0_u8; 1024];
            while !request.windows(4).any(|part| part == b"\r\n\r\n") {
                let n = peer.read(&mut chunk).await.unwrap();
                assert!(n > 0 && request.len() < 16_384);
                request.extend_from_slice(&chunk[..n]);
            }
            let headers = String::from_utf8(request).unwrap();
            assert!(headers
                .lines()
                .any(|line| line == "Authorization: Bearer refreshed"));
        });
        let (tx, _rx) = mpsc::unbounded_channel();
        let transport = Arc::new(
            HybridTransport::new(
                format!("ws://127.0.0.1:{port}/ws/worker"),
                Vec::new(),
                None,
                tx,
                Arc::new(|| vec![("Authorization".into(), "Bearer refreshed".into())]),
            )
            .unwrap(),
        );
        let runner = {
            let transport = transport.clone();
            tokio::spawn(async move {
                transport.reads.run().await;
            })
        };
        tokio::time::timeout(Duration::from_secs(3), server)
            .await
            .unwrap()
            .unwrap();
        transport.reads.close();
        tokio::time::timeout(Duration::from_secs(3), runner)
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn hybrid_close_bounds_unreachable_post_retry_window() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let hybrid = HybridTransport::new(
            "ws://127.0.0.1:9/ws/s1".into(),
            Vec::new(),
            None,
            tx,
            Arc::new(Vec::new),
        )
        .unwrap();
        hybrid
            .writes
            .write(&serde_json::json!({"type":"stream_event"}))
            .await;
        tokio::time::timeout(
            Duration::from_millis(CLOSE_GRACE_MS + 2_000),
            hybrid.close(),
        )
        .await
        .expect("Hybrid close must end despite repeated HTTP failures");
        assert!(hybrid.reads.is_closed());
    }

    #[tokio::test]
    async fn outbound_batches_and_flushes() {
        let auth: HybridAuth = Arc::new(|| vec![]);
        let outbound = HybridOutbound::new("ws://127.0.0.1:9/ws/s1", auth).unwrap();
        outbound
            .write(&serde_json::json!({"type":"stream_event"}))
            .await;
        // A stream event returns before the short batching window expires.
        outbound.close();
    }
}
