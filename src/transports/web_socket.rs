//! WebSocket transport: reads server frames and writes client frames.
//! 2.1.88 reference: `src/cli/transports/WebSocketTransport.ts`.
//!
//! Behaviour aligned with the source: reconnection with exponential backoff
//! and a 10-minute budget, permanent close codes (1002/4001/4003), system
//! sleep/wake detection, 10s ping liveness checks, 300s keep-alive data
//! frames, and a 1000-message replay buffer keyed by the server's `open`
//! frame `lastId`.
//!
//! RFC 6455 client supports cleartext ws:// and authenticated TLS wss://.

use anyhow::{anyhow, bail, Result};
use base64::Engine;
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Notify};
use tokio_rustls::{rustls, TlsConnector};

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

trait WebSocketIo: AsyncRead + AsyncWrite + Send + Unpin {}
impl<T: AsyncRead + AsyncWrite + Send + Unpin> WebSocketIo for T {}

/// RFC 6455 client connection with TLS when wss:// is selected.
pub struct MinimalWsClient {
    stream: Box<dyn WebSocketIo>,
    read_buffer: Vec<u8>,
    fragmented: Option<Vec<u8>>,
    /// Server confirmation is read from the RFC 6455 upgrade response.
    server_last_request_id: Option<String>,
}

impl MinimalWsClient {
    /// Perform the HTTP upgrade handshake over a cleartext TCP stream.
    pub async fn connect(
        host: &str,
        port: u16,
        path: &str,
        extra_headers: &[(String, String)],
        secure: bool,
    ) -> Result<Self> {
        let tcp = tokio::time::timeout(
            Duration::from_millis(CONNECT_TIMEOUT_MS),
            TcpStream::connect((host, port)),
        )
        .await
        .map_err(|_| anyhow!("WS connect timeout after {CONNECT_TIMEOUT_MS}ms"))??;
        let mut stream: Box<dyn WebSocketIo> = if secure {
            let roots =
                rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            let config = rustls::ClientConfig::builder()
                .with_root_certificates(roots)
                .with_no_client_auth();
            let server_name = rustls::pki_types::ServerName::try_from(host.to_owned())
                .map_err(|error| anyhow!("Invalid WSS server name: {error}"))?;
            let tls = tokio::time::timeout(
                Duration::from_millis(CONNECT_TIMEOUT_MS),
                TlsConnector::from(Arc::new(config)).connect(server_name, tcp),
            )
            .await
            .map_err(|_| anyhow!("WSS TLS handshake timeout"))??;
            Box::new(tls)
        } else {
            Box::new(tcp)
        };
        let mut key_bytes = [0_u8; 16];
        getrandom::getrandom(&mut key_bytes)
            .map_err(|error| anyhow!("WS handshake randomness: {error}"))?;
        let key = base64::engine::general_purpose::STANDARD.encode(key_bytes);
        let mut request = format!(
            "GET {path} HTTP/1.1\r\nHost: {host}:{port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n"
        );
        for (name, value) in extra_headers {
            if name.is_empty()
                || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                || value.contains(['\r', '\n'])
            {
                bail!("Invalid WebSocket upgrade header");
            }
            request.push_str(&format!("{name}: {value}\r\n"));
        }
        request.push_str("\r\n");
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
        let header_end = response
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .ok_or_else(|| anyhow!("WebSocket upgrade response missing terminator"))?
            + 4;
        let header = std::str::from_utf8(&response[..header_end])?;
        let status = header
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .unwrap_or("");
        if status != "101" {
            bail!("WS handshake failed with status {status}");
        }
        let mut upgrade = false;
        let mut connection_upgrade = false;
        let mut accept = None;
        let mut server_last_request_id = None;
        for line in header.split("\r\n").skip(1) {
            if let Some((header_name, value)) = line.split_once(':') {
                let value = value.trim();
                if header_name.eq_ignore_ascii_case("Upgrade") {
                    upgrade = value.eq_ignore_ascii_case("websocket");
                } else if header_name.eq_ignore_ascii_case("Connection") {
                    connection_upgrade = value
                        .split(',')
                        .any(|part| part.trim().eq_ignore_ascii_case("Upgrade"));
                } else if header_name.eq_ignore_ascii_case("Sec-WebSocket-Accept") {
                    accept = Some(value);
                } else if header_name.eq_ignore_ascii_case("X-Last-Request-Id") {
                    server_last_request_id = Some(value.to_string());
                }
            }
        }
        let expected = websocket_accept(&key);
        if !upgrade || !connection_upgrade || accept != Some(expected.as_str()) {
            bail!("WS upgrade failed RFC 6455 server verification");
        }
        Ok(Self {
            stream,
            read_buffer: response[header_end..].to_vec(),
            fragmented: None,
            server_last_request_id,
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
        getrandom::getrandom(&mut mask)
            .map_err(|error| anyhow!("WS frame masking randomness: {error}"))?;
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
            let fin = header[0] & 0x80 != 0;
            if header[0] & 0x70 != 0 {
                bail!("WS frame reserved bits must be zero");
            }
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
            validate_server_frame(fin, opcode, masked, len)?;
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
                    if self.fragmented.is_some() {
                        bail!("WS data message began before previous fragments completed");
                    }
                    if fin {
                        return Ok(WsIncoming::Text(String::from_utf8(payload)?));
                    }
                    self.fragmented = Some(payload);
                }
                0x0 => {
                    let collected = self
                        .fragmented
                        .as_mut()
                        .ok_or_else(|| anyhow!("WS continuation without initial frame"))?;
                    if collected.len().saturating_add(payload.len()) > 16 * 1024 * 1024 {
                        bail!("WS fragmented message too large");
                    }
                    collected.extend_from_slice(&payload);
                    if fin {
                        let whole = self
                            .fragmented
                            .take()
                            .ok_or_else(|| anyhow!("WS continuation state missing"))?;
                        return Ok(WsIncoming::Text(String::from_utf8(whole)?));
                    }
                }
                0x8 => {
                    if payload.len() == 1 {
                        bail!("WS protocol error: close payload cannot contain only one byte");
                    }
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
        let mut out = Vec::with_capacity(len);
        let available = len.min(self.read_buffer.len());
        out.extend(self.read_buffer.drain(..available));
        if available < len {
            let mut remaining = vec![0_u8; len - available];
            tokio::time::timeout(
                Duration::from_secs(45),
                self.stream.read_exact(&mut remaining),
            )
            .await
            .map_err(|_| anyhow!("WS read timeout"))??;
            out.extend_from_slice(&remaining);
        }
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
    last_authorization: Mutex<Option<String>>,
    closed: AtomicBool,
    shutdown: Notify,
}

/// Type of the auth header provider (fresh credentials per attempt).
pub type WsAuthHeaders = Arc<dyn Fn() -> Vec<(String, String)> + Send + Sync>;

pub struct WebSocketTransport {
    shared: Arc<WsShared>,
    #[allow(dead_code)]
    auth: Option<WsAuthHeaders>,
    outbound_tx: mpsc::Sender<Value>,
    outbound_rx: tokio::sync::Mutex<mpsc::Receiver<Value>>,
}

/// Result of one connection lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
        let (outbound_tx, outbound_rx) = mpsc::channel(DEFAULT_MAX_BUFFER_SIZE);
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
                last_authorization: Mutex::new(None),
                closed: AtomicBool::new(false),
                shutdown: Notify::new(),
            }),
            auth,
            outbound_tx,
            outbound_rx: tokio::sync::Mutex::new(outbound_rx),
        }
    }

    /// Send through the live socket while retaining FIFO and bounded backpressure.
    pub async fn queue_message(&self, message: Value) -> Result<()> {
        if self.is_closed() || self.state() == WsState::Closed {
            bail!("WebSocket transport is closed");
        }
        self.record_for_replay(&message);
        // Original write() saves UUID-bearing events when offline; transient
        // messages have no replay key and are skipped while disconnected.
        if self.state() != WsState::Connected {
            return Ok(());
        }
        self.outbound_tx
            .send(message)
            .await
            .map_err(|_| anyhow!("WebSocket outbound queue is closed"))
    }

    fn record_for_replay(&self, message: &Value) {
        if let Some(uuid) = message.get("uuid").and_then(Value::as_str) {
            self.shared
                .buffer
                .lock()
                .expect("ws buffer")
                .add(message.clone());
            self.shared.state.lock().expect("ws state").last_sent_id = Some(uuid.to_owned());
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
        let shutdown = self.shared.shutdown.notified();
        tokio::pin!(shutdown);
        shutdown.as_mut().enable();
        if self.is_closed() {
            return None;
        }
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(delay)) => {
                Some(Duration::from_millis(delay))
            }
            _ = &mut shutdown => None,
        }
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
        *self
            .shared
            .last_authorization
            .lock()
            .expect("ws authorization") = headers
            .iter()
            .rev()
            .find(|(name, _)| name.eq_ignore_ascii_case("Authorization"))
            .map(|(_, value)| value.clone());
        let mut conn = match MinimalWsClient::connect(
            &host,
            port,
            &path,
            &headers,
            self.shared.url.starts_with("wss://"),
        )
        .await
        {
            Ok(conn) => conn,
            Err(_) => return ConnectionOutcome::Reconnect,
        };
        {
            let mut state = self.shared.state.lock().expect("ws state");
            state.state = WsState::Connected;
            state.reconnect_attempts = 0;
            state.reconnect_start = None;
        }
        // Source Node/ws learns the confirmed message ID from upgrade
        // headers; Bun's no-header branch replays the entire UUID buffer.
        if self.buffered_count() > 0 {
            let confirmed = conn.server_last_request_id.clone();
            if self
                .replay_buffered_messages(&mut conn, confirmed.as_deref())
                .await
                .is_err()
            {
                return ConnectionOutcome::Reconnect;
            }
        }
        let mut last_ping = Instant::now();
        let mut pong_received = true;
        let mut last_keep_alive = Instant::now();
        let mut last_activity = Instant::now();
        let mut outbound = self.outbound_rx.lock().await;
        loop {
            if self.is_closed() {
                let _ = conn.send_close(1000).await;
                return ConnectionOutcome::Terminal;
            }
            tokio::select! {
                outbound_message = outbound.recv() => {
                    if let Some(message) = outbound_message {
                        if conn
                            .send_text(&format!("{}\n", crate::ndjson::stringify_value(&message)))
                            .await
                            .is_err()
                        {
                            return ConnectionOutcome::Reconnect;
                        }
                    }
                }
                message = conn.read_message() => {
                    match message {
                        Ok(WsIncoming::Text(text)) => {
                            last_activity = Instant::now();
                            pong_received = true;
                            // The source forwards every data frame unchanged,
                            // including JSON objects with type="open" or "close".
                            let _ = self.shared.data_tx.send(text);
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

    /// Replay buffered messages newer than `last_id` (source:
    /// `replayBufferedMessages`). Evicted-buffer bookkeeping matches the
    /// source: when the confirm point was already evicted, replay everything.
    async fn replay_buffered_messages(
        &self,
        conn: &mut MinimalWsClient,
        last_id: Option<&str>,
    ) -> Result<()> {
        let (pending, all_confirmed) = {
            let mut buffer = self.shared.buffer.lock().expect("ws buffer");
            pending_after_confirmation(&mut buffer, last_id)
        };
        if all_confirmed {
            self.shared.state.lock().expect("ws state").last_sent_id = None;
        }
        for message in &pending {
            conn.send_text(&format!("{}\n", crate::ndjson::stringify_value(message)))
                .await?;
        }
        Ok(())
    }

    /// Map a dropped connection to terminal vs reconnect, mirroring the
    /// permanent-close-code handling in the source.
    fn handle_connection_error(&self, close_code: Option<u16>) -> ConnectionOutcome {
        if close_code == Some(4003) {
            if let Some(refresh) = &self.auth {
                let fresh_authorization = refresh()
                    .into_iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case("Authorization"))
                    .map(|(_, value)| value);
                let mut current = self
                    .shared
                    .last_authorization
                    .lock()
                    .expect("ws authorization");
                if fresh_authorization.is_some() && fresh_authorization != *current {
                    *current = fresh_authorization;
                    return ConnectionOutcome::Reconnect;
                }
            }
        }
        if close_code.is_some_and(|code| PERMANENT_CLOSE_CODES.contains(&code)) {
            return ConnectionOutcome::Terminal;
        }
        ConnectionOutcome::Reconnect
    }

    /// Buffer one message and write it as a text frame. Messages keep their
    /// `uuid` for replay bookkeeping (`lastSentId`).
    pub async fn write(&self, connection: &mut MinimalWsClient, message: &Value) -> Result<()> {
        self.record_for_replay(message);
        connection
            .send_text(&format!("{}\n", crate::ndjson::stringify_value(message)))
            .await
    }

    pub fn close(&self) {
        self.shared.closed.store(true, Ordering::SeqCst);
        self.shared.shutdown.notify_waiters();
        let mut state = self.shared.state.lock().expect("ws state");
        state.state = WsState::Closed;
    }

    /// Clear the replay buffer (source: `clearMessageBuffer`).
    pub fn clear_message_buffer(&self) {
        self.shared.buffer.lock().expect("ws buffer").clear();
    }
}

/// The browser/Node WebSocket client rejects masked server frames,
/// oversized data frames and fragmented control frames per RFC 6455.
fn validate_server_frame(fin: bool, opcode: u8, masked: bool, len: u64) -> Result<()> {
    if masked {
        bail!("WS protocol error: server frames must not be masked");
    }
    if len > 16 * 1024 * 1024 {
        bail!("WS frame too large: {len} bytes");
    }
    if opcode >= 0x8 && (!fin || len > 125) {
        bail!("WS protocol error: control frames must be final and at most 125 bytes");
    }
    Ok(())
}

/// The server's last confirmed UUID is inclusive. Preserve all messages
/// when the confirmation is absent or its UUID has already been evicted.
fn pending_after_confirmation(
    buffer: &mut CircularBuffer<Value>,
    last_id: Option<&str>,
) -> (Vec<Value>, bool) {
    let messages = buffer.to_vec();
    let confirmed_count = last_id
        .and_then(|id| {
            messages
                .iter()
                .position(|message| message.get("uuid").and_then(Value::as_str) == Some(id))
        })
        .map(|index| index + 1)
        .unwrap_or(0);
    if confirmed_count > 0 {
        buffer.replace_all(messages[confirmed_count..].to_vec());
    }
    (
        messages[confirmed_count..].to_vec(),
        confirmed_count > 0 && confirmed_count == messages.len(),
    )
}

/// Compute the RFC 6455 upgrade digest (SHA-1 is prescribed by the protocol).
fn websocket_accept(key: &str) -> String {
    const GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
    let source = format!("{key}{GUID}");
    let digest = ring::digest::digest(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, source.as_bytes());
    base64::engine::general_purpose::STANDARD.encode(digest.as_ref())
}

/// Parse ws:// and wss:// URLs, preserving path, query and default ports.
pub fn parse_ws_url(url: &str) -> Result<(String, u16, String)> {
    let parsed = reqwest::Url::parse(url)?;
    if !matches!(parsed.scheme(), "ws" | "wss") {
        bail!("Unsupported WebSocket URL scheme: {}", parsed.scheme());
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        bail!("Credentials in a WebSocket URL are forbidden");
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow!("WebSocket URL missing hostname"))?
        .to_string();
    let port = parsed
        .port_or_known_default()
        .ok_or_else(|| anyhow!("WebSocket URL missing port"))?;
    let mut path = parsed.path().to_string();
    if let Some(query) = parsed.query() {
        path.push('?');
        path.push_str(query);
    }
    Ok((host, port, path))
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
    fn server_frame_validation_rejects_masked_data_and_fragmented_control() {
        assert!(validate_server_frame(true, 0x1, true, 3).is_err());
        assert!(validate_server_frame(false, 0x9, false, 1).is_err());
        assert!(validate_server_frame(true, 0x9, false, 126).is_err());
        assert!(validate_server_frame(true, 0x1, false, 16 * 1024 * 1024 + 1).is_err());
        assert!(validate_server_frame(true, 0x9, false, 125).is_ok());
    }

    #[tokio::test]
    async fn masked_server_frame_is_rejected_before_reading_payload() {
        let (local, _peer) = tokio::io::duplex(32);
        let mut socket = MinimalWsClient {
            stream: Box::new(local),
            read_buffer: vec![0x81, 0x80],
            fragmented: None,
            server_last_request_id: None,
        };
        let err = socket.read_message().await.unwrap_err();
        assert!(err.to_string().contains("server frames must not be masked"));
    }

    #[test]
    fn reconnect_ack_evicts_confirmed_messages_but_preserves_unconfirmed_tail() {
        let mut buffer = CircularBuffer::new(5);
        for id in ["a", "b", "c", "d"] {
            buffer.add(json!({"uuid": id, "type": "result"}));
        }
        let (pending, all_confirmed) = pending_after_confirmation(&mut buffer, Some("b"));
        assert!(!all_confirmed);
        assert_eq!(pending.len(), 2);
        assert_eq!(buffer.len(), 2);
        assert_eq!(buffer.to_vec(), pending);
        assert_eq!(pending[0]["uuid"], "c");
        let (remaining, all_confirmed) = pending_after_confirmation(&mut buffer, Some("d"));
        assert!(all_confirmed);
        assert!(remaining.is_empty());
        assert!(buffer.is_empty());
    }

    #[test]
    fn reconnect_unknown_ack_replays_all_without_dropping_messages() {
        let mut buffer = CircularBuffer::new(2);
        for id in ["b", "c", "d"] {
            buffer.add(json!({"uuid": id}));
        }
        let (pending, all_confirmed) = pending_after_confirmation(&mut buffer, Some("a"));
        assert!(!all_confirmed);
        assert_eq!(pending, buffer.to_vec());
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[0]["uuid"], "c");
    }

    #[test]
    fn ws_upgrade_accept_matches_rfc6455() {
        assert_eq!(
            websocket_accept("dGhlIHNhbXBsZSBub25jZQ=="),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        );
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
        assert_eq!(
            parse_ws_url("wss://host/path").unwrap(),
            ("host".to_string(), 443, "/path".to_string())
        );
        assert!(parse_ws_url("ftp://host/path").is_err());
    }

    #[tokio::test]
    async fn running_transport_delivers_queued_message_to_tcp_peer() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = tokio::spawn(async move {
            let (mut peer, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut chunk = [0_u8; 512];
            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                let n = peer.read(&mut chunk).await.unwrap();
                assert!(n > 0 && request.len() < 16384);
                request.extend_from_slice(&chunk[..n]);
            }
            let head = String::from_utf8(request).unwrap();
            let key = head
                .lines()
                .find_map(|l| l.strip_prefix("Sec-WebSocket-Key: "))
                .unwrap()
                .trim();
            let reply = format!("HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n", websocket_accept(key));
            peer.write_all(reply.as_bytes()).await.unwrap();
            let mut header = [0_u8; 2];
            peer.read_exact(&mut header).await.unwrap();
            assert_eq!(header[0], 0x81);
            assert_ne!(header[1] & 0x80, 0);
            let size = (header[1] & 0x7f) as usize;
            assert!(size < 126);
            let mut mask = [0_u8; 4];
            peer.read_exact(&mut mask).await.unwrap();
            let mut body = vec![0; size];
            peer.read_exact(&mut body).await.unwrap();
            for (i, b) in body.iter_mut().enumerate() {
                *b ^= mask[i % 4];
            }
            assert_eq!(
                body.last(),
                Some(&b'\n'),
                "WebSocket wire requires an NDJSON newline"
            );
            serde_json::from_slice::<Value>(&body).unwrap()
        });
        let (tx, _rx) = mpsc::unbounded_channel();
        let client = Arc::new(WebSocketTransport::new(
            format!("ws://127.0.0.1:{port}/session"),
            Vec::new(),
            None,
            tx,
            false,
            None,
        ));
        client
            .queue_message(json!({"type":"result","uuid":"live-one"}))
            .await
            .unwrap();
        let task = {
            let client = client.clone();
            tokio::spawn(async move { client.run().await })
        };
        let sent = tokio::time::timeout(Duration::from_secs(5), server)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(sent["uuid"], "live-one");
        client.close();
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(client.buffered_count(), 1);
        assert!(client
            .queue_message(json!({"uuid":"too-late"}))
            .await
            .is_err());
    }

    #[tokio::test]
    async fn upgrade_confirmation_replays_only_unconfirmed_and_forwards_open_payload() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = tokio::spawn(async move {
            let (mut peer, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut buf = [0_u8; 512];
            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                let n = peer.read(&mut buf).await.unwrap();
                assert!(n > 0 && request.len() < 16384);
                request.extend_from_slice(&buf[..n]);
            }
            let request = String::from_utf8(request).unwrap();
            let key = request
                .lines()
                .find_map(|line| line.strip_prefix("Sec-WebSocket-Key: "))
                .unwrap()
                .trim();
            let reply = format!(
                "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\nX-Last-Request-Id: a\r\n\r\n",
                websocket_accept(key)
            );
            peer.write_all(reply.as_bytes()).await.unwrap();
            let inbound = r#"{"type":"open","content":"application event"}"#;
            let mut frame = vec![0x81, inbound.len() as u8];
            frame.extend_from_slice(inbound.as_bytes());
            peer.write_all(&frame).await.unwrap();

            let mut header = [0_u8; 2];
            peer.read_exact(&mut header).await.unwrap();
            assert_eq!(header[0], 0x81);
            let n = (header[1] & 0x7F) as usize;
            let mut mask = [0_u8; 4];
            peer.read_exact(&mut mask).await.unwrap();
            let mut body = vec![0_u8; n];
            peer.read_exact(&mut body).await.unwrap();
            for (i, byte) in body.iter_mut().enumerate() {
                *byte ^= mask[i % 4];
            }
            serde_json::from_slice::<Value>(&body).unwrap()
        });
        let (tx, mut rx) = mpsc::unbounded_channel();
        let client = Arc::new(WebSocketTransport::new(
            format!("ws://127.0.0.1:{port}/session"),
            Vec::new(),
            None,
            tx,
            false,
            None,
        ));
        {
            let mut buffered = client.shared.buffer.lock().unwrap();
            buffered.add(json!({"type":"result","uuid":"a"}));
            buffered.add(json!({"type":"result","uuid":"b"}));
        }
        let run = {
            let client = client.clone();
            tokio::spawn(async move { client.run().await })
        };
        let observed = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("must forward open content")
            .expect("inbound payload");
        assert_eq!(
            serde_json::from_str::<Value>(&observed).unwrap()["content"],
            "application event"
        );
        let replayed = tokio::time::timeout(Duration::from_secs(5), server)
            .await
            .expect("replay arrives")
            .unwrap();
        assert_eq!(replayed["uuid"], "b");
        assert_eq!(client.buffered_count(), 1);
        client.close();
        tokio::time::timeout(Duration::from_secs(5), run)
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn failed_replay_write_surfaces_error_for_reconnect() {
        let (local, remote) = tokio::io::duplex(32);
        drop(remote);
        let mut conn = MinimalWsClient {
            stream: Box::new(local),
            read_buffer: Vec::new(),
            fragmented: None,
            server_last_request_id: None,
        };
        let (tx, _rx) = mpsc::unbounded_channel();
        let transport =
            WebSocketTransport::new("ws://127.0.0.1:1".into(), Vec::new(), None, tx, false, None);
        transport
            .shared
            .buffer
            .lock()
            .unwrap()
            .add(json!({"uuid":"unconfirmed"}));
        assert!(transport
            .replay_buffered_messages(&mut conn, None)
            .await
            .is_err());
        assert_eq!(
            transport.buffered_count(),
            1,
            "unconfirmed message remains queued"
        );
    }

    #[test]
    fn unauthorized_close_reconnects_only_for_a_new_authorization_token() {
        let token = Arc::new(Mutex::new(String::from("Bearer old")));
        let captured = token.clone();
        let refresh: WsAuthHeaders =
            Arc::new(move || vec![("Authorization".to_owned(), captured.lock().unwrap().clone())]);
        let (data_tx, _data_rx) = mpsc::unbounded_channel();
        let client = WebSocketTransport::new(
            "ws://127.0.0.1:1/x".into(),
            Vec::new(),
            None,
            data_tx,
            true,
            Some(refresh),
        );
        *client.shared.last_authorization.lock().unwrap() = Some("Bearer old".into());
        assert_eq!(
            client.handle_connection_error(Some(4003)),
            ConnectionOutcome::Terminal
        );
        *token.lock().unwrap() = String::from("Bearer refreshed");
        assert_eq!(
            client.handle_connection_error(Some(4003)),
            ConnectionOutcome::Reconnect
        );
        assert_eq!(
            client.handle_connection_error(Some(4003)),
            ConnectionOutcome::Terminal
        );
        assert_eq!(
            client.handle_connection_error(Some(4001)),
            ConnectionOutcome::Terminal
        );
    }

    #[tokio::test]
    async fn close_interrupts_websocket_reconnect_backoff() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let transport = Arc::new(WebSocketTransport::new(
            "ws://127.0.0.1:0/unreachable".into(),
            Vec::new(),
            None,
            tx,
            true,
            None,
        ));
        let pending = {
            let transport = Arc::clone(&transport);
            tokio::spawn(async move { transport.schedule_reconnect().await })
        };
        tokio::time::sleep(Duration::from_millis(30)).await;
        transport.close();
        let result = tokio::time::timeout(Duration::from_millis(500), pending)
            .await
            .expect("close must wake the reconnect timer")
            .unwrap();
        assert!(result.is_none());
        assert_eq!(transport.state(), WsState::Closed);
    }

    #[tokio::test]
    async fn offline_messages_follow_uuid_only_replay_contract() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let transport = WebSocketTransport::new(
            "ws://127.0.0.1:1/x".into(),
            Vec::new(),
            None,
            tx,
            false,
            None,
        );
        transport
            .queue_message(json!({"type":"transient"}))
            .await
            .unwrap();
        assert_eq!(transport.buffered_count(), 0);
        transport
            .queue_message(json!({"type":"result","uuid":"persistent"}))
            .await
            .unwrap();
        assert_eq!(transport.buffered_count(), 1);
        assert_eq!(
            transport.shared.buffer.lock().unwrap().to_vec()[0]["uuid"],
            "persistent"
        );
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
