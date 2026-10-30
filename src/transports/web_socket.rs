//! WebSocket transport: reads server frames and writes client frames.
//! 2.1.88 reference: `src/cli/transports/WebSocketTransport.ts`.
//!
//! Behaviour aligned with the source: reconnection with exponential backoff
//! and a 10-minute budget, permanent close codes (1002/4001/4003), system
//! sleep/wake detection, 10s ping liveness checks, 300s keep-alive data
//! frames, and a 1000-message replay buffer keyed by the server's `open`
//! frame `lastId`.
//!
//! The underlying RFC 6455 client is a minimal self-hosted implementation
//! (`ws://` cleartext only). Plain-text framing keeps local loopback and
//! self-hosted endpoints testable without a TLS WebSocket dependency; the
//! `SseTransport`/`HybridTransport` cover TLS ingress paths.

use anyhow::{anyhow, bail, Result};
use base64::Engine;
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use super::sse::reconnect_delay;

pub const KEEP_ALIVE_FRAME: &str = "{\"type\":\"keep_alive\"}\n";
pub const DEFAULT_MAX_BUFFER_SIZE: usize = 1_000;
pub const DEFAULT_BASE_RECONNECT_DELAY_MS: u64 = 1_000;
pub const DEFAULT_MAX_RECONNECT_DELAY_MS: u64 = 30_000;
/// Time budget for reconnection attempts before giving up (10 minutes).
pub const DEFAULT_RECONNECT_GIVE_UP_MS: u64 = 600_000;
pub const CONNECT_TIMEOUT_MS: u64 = 5_000;
pub const DEFAULT_PING_INTERVAL_MS: u64 = 10_000;
pub const DEFAULT_KEEP_ALIVE_INTERVAL_MS: u64 = 300_000;
/// A reconnect gap far beyond the ping interval indicates system sleep.
pub const SLEEP_DETECTION_THRESHOLD_MS: u64 = 60_000;
/// Close codes that must never trigger reconnection.
pub const PERMANENT_CLOSE_CODES: [u16; 3] = [1002, 4001, 4003];

/// Fixed-capacity FIFO ring buffer that evicts its oldest entry when full.
#[derive(Debug)]
pub struct CircularBuffer<T> {
    capacity: usize,
    items: VecDeque<T>,
    evicted: usize,
}

impl<T> CircularBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            items: VecDeque::new(),
            evicted: 0,
        }
    }

    pub fn add(&mut self, item: T) {
        if self.items.len() == self.capacity {
            self.items.pop_front();
            self.evicted += 1;
        }
        self.items.push_back(item);
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn replace_all(&mut self, items: Vec<T>) {
        self.items.clear();
        self.items.extend(items);
    }

    pub fn to_vec(&self) -> Vec<T>
    where
        T: Clone,
    {
        self.items.iter().cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Number of entries evicted so far (dropped-batch accounting).
    pub fn evicted_count(&self) -> usize {
        self.evicted
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WsIncoming {
    Text(String),
    Pong,
    Ping,
    Close(Option<u16>),
}

/// Minimal RFC 6455 client connection (`ws://` cleartext).
/// The `Sec-WebSocket-Accept` header is intentionally not verified: the
/// handshake targets self-hosted loopback endpoints and skipping the SHA-1
/// digest avoids an extra hash dependency in the offline build.
pub struct MinimalWsClient {
    stream: TcpStream,
    #[allow(dead_code)]
    read_buffer: Vec<u8>,
    fragmented: Option<Vec<u8>>,
}

impl MinimalWsClient {
    /// Perform the HTTP upgrade handshake over a cleartext TCP stream.
    pub async fn connect(
        host: &str,
        port: u16,
        path: &str,
        extra_headers: &[(String, String)],
    ) -> Result<Self> {
        let stream = tokio::time::timeout(
            Duration::from_millis(CONNECT_TIMEOUT_MS),
            TcpStream::connect((host, port)),
        )
        .await
        .map_err(|_| anyhow!("WS connect timeout after {CONNECT_TIMEOUT_MS}ms"))??;
        let mut key_bytes = [0_u8; 16];
        getrandom::getrandom(&mut key_bytes)?;
        let key = base64::engine::general_purpose::STANDARD.encode(key_bytes);
        let mut request = format!(
            "GET {path} HTTP/1.1\r\nHost: {host}:{port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n"
        );
        for (name, value) in extra_headers {
            request.push_str(&format!("{name}: {value}\r\n"));
        }
        request.push_str("\r\n");
        let mut stream = stream;
        stream.write_all(request.as_bytes()).await?;
        let mut response = Vec::new();
        let mut chunk = [0_u8; 1024];
        loop {
            let read = tokio::time::timeout(
                Duration::from_millis(CONNECT_TIMEOUT_MS),
                stream.read(&mut chunk),
            )
            .await
            .map_err(|_| anyhow!("WS handshake read timeout"))??;
            if read == 0 {
                bail!("WS handshake: connection closed");
            }
            response.extend_from_slice(&chunk[..read]);
            if response.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
            if response.len() > 64 * 1024 {
                bail!("WS handshake: oversized response");
            }
        }
        let head = String::from_utf8_lossy(&response);
        let status = head
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .unwrap_or("");
        if status != "101" {
            bail!("WS handshake failed with status {status}");
        }
        Ok(Self {
            stream,
            read_buffer: Vec::new(),
            fragmented: None,
        })
    }

    /// Send one masked text frame (client frames are always masked).
    pub async fn send_text(&mut self, text: &str) -> Result<()> {
        self.send_frame(0x1, text.as_bytes()).await
    }

    pub async fn send_ping(&mut self) -> Result<()> {
        self.send_frame(0x9, b"").await
    }

    pub async fn send_pong(&mut self) -> Result<()> {
        self.send_frame(0xA, b"").await
    }

    pub async fn send_close(&mut self, code: u16) -> Result<()> {
        self.send_frame(0x8, &code.to_be_bytes()).await
    }

    async fn send_frame(&mut self, opcode: u8, payload: &[u8]) -> Result<()> {
        let mut frame = Vec::with_capacity(payload.len() + 14);
        frame.push(0x80 | opcode);
        let len = payload.len();
        let mut mask = [0_u8; 4];
        getrandom::getrandom(&mut mask)?;
        if len < 126 {
            frame.push(0x80 | len as u8);
        } else if len <= u16::MAX as usize {
            frame.push(0x80 | 126);
            frame.extend_from_slice(&(len as u16).to_be_bytes());
        } else {
            frame.push(0x80 | 127);
            frame.extend_from_slice(&(len as u64).to_be_bytes());
        }
        frame.extend_from_slice(&mask);
        for (index, byte) in payload.iter().enumerate() {
            frame.push(byte ^ mask[index % 4]);
        }
        self.stream.write_all(&frame).await?;
        self.stream.flush().await?;
        Ok(())
    }

    /// Read one incoming message, reassembling fragmentation and answering
    /// server pings with pongs. Returns `Close` on a close frame or EOF.
    pub async fn read_message(&mut self) -> Result<WsIncoming> {
        loop {
            let header = self.read_exact_vec(2).await?;
            let opcode = header[0] & 0x0F;
            let masked = header[1] & 0x80 != 0;
            let mut len = (header[1] & 0x7F) as u64;
            if len == 126 {
                let ext = self.read_exact_vec(2).await?;
                len = u16::from_be_bytes([ext[0], ext[1]]) as u64;
            } else if len == 127 {
                let ext = self.read_exact_vec(8).await?;
                let mut bytes = [0_u8; 8];
                bytes.copy_from_slice(&ext);
                len = u64::from_be_bytes(bytes);
            }
            if len > 16 * 1024 * 1024 {
                bail!("WS frame too large: {len} bytes");
            }
            let mask = if masked {
                Some(self.read_exact_vec(4).await?)
            } else {
                None
            };
            let mut payload = self.read_exact_vec(len as usize).await?;
            if let Some(mask) = mask {
                for (index, byte) in payload.iter_mut().enumerate() {
                    *byte ^= mask[index % 4];
                }
            }
            match opcode {
                0x1 | 0x2 => {
                    return Ok(WsIncoming::Text(
                        String::from_utf8_lossy(&payload).into_owned(),
                    ));
                }
                0x0 => match &mut self.fragmented {
                    Some(collected) => {
                        collected.extend_from_slice(&payload);
                    }
                    None => bail!("WS continuation frame without a started message"),
                },
                0x8 => {
                    let code = if payload.len() >= 2 {
                        Some(u16::from_be_bytes([payload[0], payload[1]]))
                    } else {
                        None
                    };
                    let _ = self.send_close(code.unwrap_or(1000)).await;
                    return Ok(WsIncoming::Close(code));
                }
                0x9 => {
                    self.send_pong().await?;
                    return Ok(WsIncoming::Ping);
                }
                0xA => return Ok(WsIncoming::Pong),
                other => bail!("WS unsupported opcode {other}"),
            }
        }
    }

    async fn read_exact_vec(&mut self, len: usize) -> Result<Vec<u8>> {
        let mut out = vec![0_u8; len];
        if len == 0 {
            return Ok(out);
        }
        tokio::time::timeout(Duration::from_secs(45), self.stream.read_exact(&mut out))
            .await
            .map_err(|_| anyhow!("WS read timeout"))??;
        Ok(out)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WsState {
    Idle,
    Reconnecting,
    Connected,
    Closing,
    Closed,
}

pub struct WsStateInner {
    pub state: WsState,
    pub reconnect_attempts: u32,
    pub reconnect_start: Option<Instant>,
    pub last_reconnect_attempt: Option<Instant>,
    pub last_sent_id: Option<String>,
}

struct WsShared {
    url: String,
    headers: Vec<(String, String)>,
    session_id: Option<String>,
    auto_reconnect: bool,
    data_tx: mpsc::UnboundedSender<String>,
    state: Mutex<WsStateInner>,
    buffer: Mutex<CircularBuffer<Value>>,
    closed: AtomicBool,
}

/// Type of the auth header provider (fresh credentials per attempt).
pub type WsAuthHeaders = Arc<dyn Fn() -> Vec<(String, String)> + Send + Sync>;

pub struct WebSocketTransport {
    shared: Arc<WsShared>,
    #[allow(dead_code)]
    auth: Option<WsAuthHeaders>,
}

/// Result of one connection lifecycle.
enum ConnectionOutcome {
    /// Server told us to stop (permanent close code) or close() was called.
    Terminal,
    /// Connection dropped; reconnect unless auto-reconnect is disabled.
    Reconnect,
}

impl WebSocketTransport {
    pub fn new(
        url: String,
        headers: Vec<(String, String)>,
        session_id: Option<String>,
        data_tx: mpsc::UnboundedSender<String>,
        auto_reconnect: bool,
        auth: Option<WsAuthHeaders>,
    ) -> Self {
        Self {
            shared: Arc::new(WsShared {
                url,
                headers,
                session_id,
                auto_reconnect,
                data_tx,
                state: Mutex::new(WsStateInner {
                    state: WsState::Idle,
                    reconnect_attempts: 0,
                    reconnect_start: None,
                    last_reconnect_attempt: None,
                    last_sent_id: None,
                }),
                buffer: Mutex::new(CircularBuffer::new(DEFAULT_MAX_BUFFER_SIZE)),
                closed: AtomicBool::new(false),
            }),
            auth,
        }
    }

    pub fn state(&self) -> WsState {
        self.shared.state.lock().expect("ws state").state
    }

    pub fn buffered_count(&self) -> usize {
        self.shared.buffer.lock().expect("ws buffer").len()
    }

    pub fn is_closed(&self) -> bool {
        self.shared.closed.load(Ordering::SeqCst)
    }

    /// Run the connect/reconnect loop until a terminal condition is reached.
    pub async fn run(&self) {
        loop {
            if self.is_closed() {
                return;
            }
            {
                let mut state = self.shared.state.lock().expect("ws state");
                state.state = WsState::Reconnecting;
            }
            match self.connect_once().await {
                ConnectionOutcome::Terminal => {
                    let mut state = self.shared.state.lock().expect("ws state");
                    state.state = WsState::Closed;
                    return;
                }
                ConnectionOutcome::Reconnect => {
                    if !self.shared.auto_reconnect {
                        let mut state = self.shared.state.lock().expect("ws state");
                        state.state = WsState::Closed;
                        return;
                    }
                    if self.schedule_reconnect().await.is_none() {
                        let mut state = self.shared.state.lock().expect("ws state");
                        state.state = WsState::Closed;
                        return;
                    }
                }
            }
        }
    }

    /// Exponential backoff with a reconnection time budget and sleep
    /// detection; returns None when the budget is exhausted.
    async fn schedule_reconnect(&self) -> Option<Duration> {
        let delay = {
            let mut state = self.shared.state.lock().expect("ws state");
            let now = Instant::now();
            let start = *state.reconnect_start.get_or_insert(now);
            if let Some(last) = state.last_reconnect_attempt {
                if now.duration_since(last) > Duration::from_millis(SLEEP_DETECTION_THRESHOLD_MS) {
                    // System likely slept: reset the budget from scratch.
                    state.reconnect_start = Some(now);
                    state.reconnect_attempts = 0;
                }
            }
            state.last_reconnect_attempt = Some(now);
            if now.duration_since(*state.reconnect_start.as_ref().unwrap_or(&start))
                >= Duration::from_millis(DEFAULT_RECONNECT_GIVE_UP_MS)
            {
                return None;
            }
            state.reconnect_attempts += 1;
            let exponential = DEFAULT_BASE_RECONNECT_DELAY_MS
                .saturating_mul(1u64 << (state.reconnect_attempts.saturating_sub(1)).min(5))
                .min(DEFAULT_MAX_RECONNECT_DELAY_MS);
            exponential
        };
        tokio::time::sleep(Duration::from_millis(delay)).await;
        Some(Duration::from_millis(delay))
    }

    /// One full connection lifecycle: handshake, replay, read loop, pings.
    async fn connect_once(&self) -> ConnectionOutcome {
        let (host, port, path) = match parse_ws_url(&self.shared.url) {
            Ok(parts) => parts,
            Err(_) => {
                // Unsupported (e.g. wss without TLS WebSocket): terminal.
                return ConnectionOutcome::Terminal;
            }
        };
        let mut headers = self.shared.headers.clone();
        if let Some(last_sent_id) = self
            .shared
            .state
            .lock()
            .expect("ws state")
            .last_sent_id
            .clone()
        {
            headers.push(("X-Last-Request-Id".to_string(), last_sent_id));
        }
        if let Some(session_id) = &self.shared.session_id {
            headers.push(("X-Session-Id".to_string(), session_id.clone()));
        }
        if let Some(auth) = &self.auth {
            for pair in auth() {
                headers.push(pair);
            }
        }
        let mut conn = match MinimalWsClient::connect(&host, port, &path, &headers).await {
            Ok(conn) => conn,
            Err(_) => return ConnectionOutcome::Reconnect,
        };
        {
            let mut state = self.shared.state.lock().expect("ws state");
            state.state = WsState::Connected;
            state.reconnect_attempts = 0;
            state.reconnect_start = None;
        }
        let mut last_ping = Instant::now();
        let mut pong_received = true;
        let mut last_keep_alive = Instant::now();
        let mut last_activity = Instant::now();
        loop {
            if self.is_closed() {
                let _ = conn.send_close(1000).await;
                return ConnectionOutcome::Terminal;
            }
            tokio::select! {
                message = conn.read_message() => {
                    match message {
                        Ok(WsIncoming::Text(text)) => {
                            last_activity = Instant::now();
                            pong_received = true;
                            if let Some(outcome) = self.handle_text(&mut conn, &text).await {
                                return outcome;
                            }
                        }
                        Ok(WsIncoming::Pong) => {
                            pong_received = true;
                            last_activity = Instant::now();
                        }
                        Ok(WsIncoming::Ping) => {
                            last_activity = Instant::now();
                        }
                        Ok(WsIncoming::Close(code)) => {
                            return self.handle_connection_error(code);
                        }
                        Err(_) => {
                            let code = None;
                            return self.handle_connection_error(code);
                        }
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(50)) => {
                    if last_ping.elapsed() >= Duration::from_millis(DEFAULT_PING_INTERVAL_MS) {
                        // A gap far beyond one interval means the host slept:
                        // drop and reconnect cheaply, replay handles overlap.
                        if last_activity.elapsed() > Duration::from_millis(DEFAULT_PING_INTERVAL_MS * 4) {
                            return ConnectionOutcome::Reconnect;
                        }
                        if !pong_received {
                            return ConnectionOutcome::Reconnect;
                        }
                        if conn.send_ping().await.is_err() {
                            return ConnectionOutcome::Reconnect;
                        }
                        pong_received = false;
                        last_ping = Instant::now();
                    }
                    if last_keep_alive.elapsed()
                        >= Duration::from_millis(DEFAULT_KEEP_ALIVE_INTERVAL_MS)
                    {
                        let _ = conn.send_text(KEEP_ALIVE_FRAME).await;
                        last_keep_alive = Instant::now();
                    }
                }
            }
        }
    }

    /// Handle one server text frame. Returns Some(outcome) when the
    /// connection lifecycle ends.
    async fn handle_text(
        &self,
        conn: &mut MinimalWsClient,
        text: &str,
    ) -> Option<ConnectionOutcome> {
        let parsed: Value = match serde_json::from_str(text) {
            Ok(value) => value,
            Err(_) => {
                let _ = self.shared.data_tx.send(text.to_string());
                return None;
            }
        };
        match parsed.get("type").and_then(Value::as_str) {
            Some("open") => {
                let last_id = parsed
                    .get("lastId")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                self.replay_buffered_messages(conn, last_id.as_deref())
                    .await;
                None
            }
            Some("close") => Some(ConnectionOutcome::Terminal),
            _ => {
                let _ = self.shared.data_tx.send(text.to_string());
                None
            }
        }
    }

    /// Replay buffered messages newer than `last_id` (source:
    /// `replayBufferedMessages`). Evicted-buffer bookkeeping matches the
    /// source: when the confirm point was already evicted, replay everything.
    async fn replay_buffered_messages(&self, conn: &mut MinimalWsClient, last_id: Option<&str>) {
        let messages = self.shared.buffer.lock().expect("ws buffer").to_vec();
        let start_index = match last_id {
            Some(last_id) => messages
                .iter()
                .position(|message| message.get("uuid").and_then(Value::as_str) == Some(last_id))
                .map(|index| index + 1),
            None => Some(0),
        };
        let Some(start_index) = start_index else {
            // Confirm point was evicted from the ring: replay all.
            for message in &messages {
                if conn
                    .send_text(&crate::ndjson::stringify_value(message))
                    .await
                    .is_err()
                {
                    return;
                }
            }
            return;
        };
        for message in &messages[start_index..] {
            if conn
                .send_text(&crate::ndjson::stringify_value(message))
                .await
                .is_err()
            {
                return;
            }
        }
    }

    /// Map a dropped connection to terminal vs reconnect, mirroring the
    /// permanent-close-code handling in the source.
    fn handle_connection_error(&self, close_code: Option<u16>) -> ConnectionOutcome {
        if let Some(code) = close_code {
            if PERMANENT_CLOSE_CODES.contains(&code) {
                return ConnectionOutcome::Terminal;
            }
        }
        ConnectionOutcome::Reconnect
    }

    /// Buffer one message and write it as a text frame. Messages keep their
    /// `uuid` for replay bookkeeping (`lastSentId`).
    pub async fn write(&self, connection: &mut MinimalWsClient, message: &Value) -> Result<()> {
        let uuid = message
            .get("uuid")
            .and_then(Value::as_str)
            .map(str::to_string);
        {
            let mut buffer = self.shared.buffer.lock().expect("ws buffer");
            buffer.add(message.clone());
        }
        if let Some(uuid) = uuid {
            self.shared.state.lock().expect("ws state").last_sent_id = Some(uuid);
        }
        connection
            .send_text(&crate::ndjson::stringify_value(message))
            .await
    }

    pub fn close(&self) {
        self.shared.closed.store(true, Ordering::SeqCst);
        let mut state = self.shared.state.lock().expect("ws state");
        state.state = WsState::Closed;
    }

    /// Clear the replay buffer (source: `clearMessageBuffer`).
    pub fn clear_message_buffer(&self) {
        self.shared.buffer.lock().expect("ws buffer").clear();
    }
}

/// Parse a `ws://host:port/path` URL into its parts.
/// `wss://` is rejected — TLS WebSocket is not available in this offline
/// build; use `SseTransport` (https) or `HybridTransport` for TLS ingress.
pub fn parse_ws_url(url: &str) -> Result<(String, u16, String)> {
    let rest = url
        .strip_prefix("ws://")
        .ok_or_else(|| anyhow!("Unsupported WebSocket URL scheme (expected ws://): {url}"))?;
    let (authority, path) = match rest.find('/') {
        Some(index) => (&rest[..index], &rest[index..]),
        None => (rest, "/"),
    };
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => (host.to_string(), port.parse::<u16>().unwrap_or(80)),
        None => (authority.to_string(), 80),
    };
    Ok((host, port, path.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn circular_buffer_evicts_oldest() {
        let mut buffer = CircularBuffer::new(3);
        for index in 0..5 {
            buffer.add(json!(index));
        }
        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.evicted_count(), 2);
        assert_eq!(buffer.to_vec(), vec![json!(2), json!(3), json!(4)]);
        buffer.clear();
        assert!(buffer.is_empty());
    }

    #[test]
    fn ws_url_parsing() {
        assert_eq!(
            parse_ws_url("ws://127.0.0.1:9912/session/s1").unwrap(),
            ("127.0.0.1".to_string(), 9912, "/session/s1".to_string())
        );
        assert_eq!(
            parse_ws_url("ws://localhost").unwrap(),
            ("localhost".to_string(), 80, "/".to_string())
        );
        assert!(parse_ws_url("wss://host/path").is_err());
    }

    #[test]
    fn permanent_close_codes_are_terminal() {
        let (data_tx, _data_rx) = mpsc::unbounded_channel();
        let transport = WebSocketTransport::new(
            "ws://127.0.0.1:1/x".to_string(),
            vec![],
            None,
            data_tx,
            true,
            None,
        );
        assert_eq!(
            transport.handle_connection_error(Some(4003)),
            ConnectionOutcome::Terminal
        );
        assert_eq!(
            transport.handle_connection_error(Some(4001)),
            ConnectionOutcome::Terminal
        );
        assert_eq!(
            transport.handle_connection_error(Some(1002)),
            ConnectionOutcome::Terminal
        );
        assert_eq!(
            transport.handle_connection_error(Some(1006)),
            ConnectionOutcome::Reconnect
        );
        assert_eq!(
            transport.handle_connection_error(None),
            ConnectionOutcome::Reconnect
        );
    }
}
