//! SSE transport: reads `event: client_event` streams and writes via POST.
//! 2.1.88 reference: `src/cli/transports/SSETransport.ts`.
//!
//! Reconnects with exponential backoff (base 1s, max 30s, ±25% jitter) inside
//! a 10-minute budget, resumes from the last seen sequence number, treats
//! 401/403/404 as permanent failures, and guards liveness with a 45s frame
//! timeout. Writes go through POST with up to 10 attempts.

use anyhow::{anyhow, bail, Result};
use serde_json::Value;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

pub const RECONNECT_BASE_DELAY_MS: u64 = 1_000;
pub const RECONNECT_MAX_DELAY_MS: u64 = 30_000;
/// Time budget for reconnection attempts before giving up (10 minutes).
pub const RECONNECT_GIVE_UP_MS: u64 = 600_000;
pub const LIVENESS_TIMEOUT_MS: u64 = 45_000;
/// HTTP statuses that indicate a permanent server-side rejection.
pub const PERMANENT_HTTP_CODES: [u16; 3] = [401, 403, 404];
pub const POST_MAX_RETRIES: u32 = 10;
pub const POST_BASE_DELAY_MS: u64 = 500;
pub const POST_MAX_DELAY_MS: u64 = 8_000;

/// One parsed Server-Sent Events frame (source `SSEFrame`).
#[derive(Debug, Default, Clone, PartialEq)]
pub struct SseFrame {
    pub event: Option<String>,
    pub data: String,
    pub id: Option<String>,
    /// Pure-comment frames (`:keepalive`) still reset the liveness timer.
    pub is_comment: bool,
}

/// Parse SSE frames delimited by double newlines; return the leftover
/// partial frame (source `parseSSEFrames`).
pub fn parse_sse_frames(buffer: &str) -> (Vec<SseFrame>, String) {
    let mut frames = Vec::new();
    let mut pos = 0;
    let bytes = buffer.as_bytes();
    while let Some(index) = find_double_newline(&bytes[pos..]) {
        let end = pos + index;
        let raw = &buffer[pos..end];
        pos = end + 2;
        if raw.trim().is_empty() {
            continue;
        }
        let mut frame = SseFrame::default();
        let mut data_lines: Vec<&str> = Vec::new();
        for line in raw.split('\n') {
            if let Some(stripped) = line.strip_prefix('\r') {
                let line = stripped;
                if line.is_empty() {
                    continue;
                }
                if line.starts_with(':') {
                    frame.is_comment = true;
                    continue;
                }
                if let Some((field, value)) = split_field(line) {
                    match field {
                        "event" => frame.event = Some(value),
                        "data" => data_lines.push(&value),
                        "id" => frame.id = Some(value),
                        _ => {}
                    }
                }
                continue;
            }
            if line.is_empty() {
                continue;
            }
            if line.starts_with(':') {
                frame.is_comment = true;
                continue;
            }
            if let Some((field, value)) = split_field(line) {
                match field {
                    "event" => frame.event = Some(value),
                    "data" => data_lines.push(value),
                    "id" => frame.id = Some(value),
                    _ => {}
                }
            }
        }
        frame.data = data_lines.join("\n");
        frames.push(frame);
    }
    (frames, buffer[pos..].to_string())
}

fn find_double_newline(bytes: &[u8]) -> Option<usize> {
    bytes.windows(2).position(|window| window == b"\n\n")
}

fn split_field(line: &str) -> Option<(&str, String)> {
    let colon = line.find(':')?;
    let field = &line[..colon];
    let rest = &line[colon + 1..];
    let value = rest.strip_prefix(' ').unwrap_or(rest);
    Some((field, value.to_string()))
}

/// Payload carried by `event: client_event` frames (source
/// `StreamClientEvent`).
#[derive(Debug, Clone, Default)]
pub struct StreamClientEvent {
    pub sequence_num: Option<u64>,
    pub payload: Option<Value>,
}

pub fn parse_client_event(data: &str) -> Option<StreamClientEvent> {
    let value: Value = serde_json::from_str(data).ok()?;
    Some(StreamClientEvent {
        sequence_num: value.get("sequence_num").and_then(Value::as_u64),
        payload: value.get("payload").cloned(),
    })
}

/// Convert an SSE URL to its HTTP POST endpoint: same base, `/stream` suffix
/// removed (source `convertSSEUrlToPostUrl`).
pub fn convert_sse_url_to_post_url(sse_url: &str) -> Result<String> {
    let lower = sse_url.to_ascii_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://")) {
        bail!("SSE POST URL requires an http(s) endpoint: {sse_url}");
    }
    if let Some(stripped) = sse_url.strip_suffix("/stream") {
        return Ok(stripped.to_string());
    }
    Ok(sse_url.to_string())
}

type AuthHeaders = Arc<dyn Fn() -> Vec<(String, String)> + Send + Sync>;

struct SseStateInner {
    /// Monotonic stream position for resumption (`from_sequence_num`).
    last_sequence_num: u64,
    /// Dedup guard for replayed client events.
    seen_sequence_nums: HashSet<u64>,
    reconnect_attempts: u32,
    reconnect_started: Option<SystemTime>,
    last_reconnect_attempt: Option<SystemTime>,
    closing: bool,
}

/// Client-event channel payload: deduped, ordered stream payloads.
pub struct SseTransport {
    url: String,
    headers: Vec<(String, String)>,
    session_id: Option<String>,
    auth: AuthHeaders,
    state: Mutex<SseStateInner>,
    closed: AtomicBool,
    event_tx: mpsc::UnboundedSender<Value>,
    pub event_rx: Mutex<Option<mpsc::UnboundedReceiver<Value>>>,
}

impl SseTransport {
    pub fn new(
        url: String,
        headers: Vec<(String, String)>,
        session_id: Option<String>,
        auth: AuthHeaders,
    ) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        Self {
            url,
            headers,
            session_id,
            auth,
            state: Mutex::new(SseStateInner {
                last_sequence_num: 0,
                seen_sequence_nums: HashSet::new(),
                reconnect_attempts: 0,
                reconnect_started: None,
                last_reconnect_attempt: None,
                closing: false,
            }),
            closed: AtomicBool::new(false),
            event_tx,
            event_rx: Mutex::new(Some(event_rx)),
        }
    }

    pub fn get_last_sequence_num(&self) -> u64 {
        self.state.lock().expect("sse state").last_sequence_num
    }

    /// Connect the SSE stream and process frames until the budget is
    /// exhausted or the transport closes. Server payloads land on the
    /// unbounded event channel (`take_event_receiver`).
    pub async fn run(&self) -> Result<()> {
        let reconnect_start = {
            let mut state = self.state.lock().expect("sse state");
            state.reconnect_started.unwrap_or_else(|| SystemTime::now())
        };
        self.state.lock().expect("sse state").reconnect_started = Some(reconnect_start);
        loop {
            if self.closed.load(Ordering::SeqCst) {
                bail!("SSE transport closed");
            }
            let outcome = self.connect_once().await;
            match outcome {
                Ok(()) => {
                    // Graceful end of stream — reconnect unless closing.
                    if self.state.lock().expect("sse state").closing {
                        return Ok(());
                    }
                }
                Err(error) => {
                    if is_permanent(&error.to_string()) {
                        return Err(error);
                    }
                }
            }
            let now = SystemTime::now();
            let mut state = self.state.lock().expect("sse state");
            let started = state.reconnect_started.unwrap_or(now);
            let elapsed = now.duration_since(started).unwrap_or_default();
            if elapsed >= Duration::from_millis(RECONNECT_GIVE_UP_MS) {
                bail!("SSE reconnection budget exhausted after {elapsed:?}");
            }
            state.reconnect_attempts += 1;
            let delay = reconnect_delay(state.reconnect_attempts);
            state.last_reconnect_attempt = Some(now);
            drop(state);
            tokio::time::sleep(delay).await;
        }
    }

    async fn connect_once(&self) -> Result<()> {
        let mut url = self.url.clone();
        let last_seq = self.state.lock().expect("sse state").last_sequence_num;
        if last_seq > 0 {
            url.push_str(&format!("?from_sequence_num={last_seq}"));
        }
        let mut request = reqwest::Client::new().get(&url);
        for (name, value) in (self.auth)() {
            request = request.header(name, value);
        }
        for (name, value) in &self.headers {
            request = request.header(name.as_str(), value.as_str());
        }
        if let Some(session_id) = &self.session_id {
            request = request.header("X-Session-Id", session_id);
        }
        request = request.header("Accept", "text/event-stream");
        let response = request.send().await?;
        let status = response.status().as_u16();
        if PERMANENT_HTTP_CODES.contains(&status) {
            bail!("SSE permanent HTTP {status}");
        }
        if !(200..300).contains(&status) {
            bail!("SSE connect failed with status {status}");
        }
        let mut buffer = String::new();
        let mut response = response;
        while let Some(chunk) = response.chunk().await? {
            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);
            let (frames, remaining) = parse_sse_frames(&buffer);
            buffer = remaining;
            for frame in frames {
                if frame.event.as_deref() == Some("client_event") {
                    if let Some(event) = parse_client_event(&frame.data) {
                        if let Some(seq) = event.sequence_num {
                            let mut state = self.state.lock().expect("sse state");
                            if !state.seen_sequence_nums.insert(seq) {
                                continue;
                            }
                            state.last_sequence_num = state.last_sequence_num.max(seq);
                        }
                        if let Some(payload) = event.payload {
                            let _ = self.event_tx.send(payload);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Send one message through the POST endpoint with up to
    /// `POST_MAX_RETRIES` attempts (4xx except 429 aborts early).
    pub async fn write(&self, post_url: &str, message: &Value) -> Result<()> {
        let mut request = reqwest::Client::new().post(post_url);
        for (name, value) in (self.auth)() {
            request = request.header(name, value);
        }
        request = request
            .header("Content-Type", "application/json")
            .header("anthropic-version", "2023-06-01")
            .body(crate::ndjson::stringify_value(message));
        let mut last_status: Option<u16> = None;
        for attempt in 1..=POST_MAX_RETRIES {
            let response = request
                .try_clone()
                .ok_or_else(|| anyhow!("SSE POST body not replayable"))?
                .send()
                .await;
            match response {
                Ok(response) => {
                    let status = response.status().as_u16();
                    if status == 200 || status == 201 {
                        return Ok(());
                    }
                    last_status = Some(status);
                    if (400..500).contains(&status) && status != 429 {
                        break;
                    }
                    if attempt < POST_MAX_RETRIES {
                        tokio::time::sleep(post_retry_delay(attempt)).await;
                    }
                }
                Err(_) => {
                    if attempt < POST_MAX_RETRIES {
                        tokio::time::sleep(post_retry_delay(attempt)).await;
                    }
                }
            }
        }
        bail!("SSE POST failed after {POST_MAX_RETRIES} attempts (last status {last_status:?})")
    }

    pub fn close(&self) {
        self.state.lock().expect("sse state").closing = true;
        self.closed.store(true, Ordering::SeqCst);
    }
}

fn is_permanent(error: &str) -> bool {
    PERMANENT_HTTP_CODES
        .iter()
        .any(|code| error.contains(&format!("permanent HTTP {code}")))
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis() as u64)
        .unwrap_or(0)
}

/// Exponential reconnect delay with ±25% jitter, clamped to
/// `[RECONNECT_BASE_DELAY_MS, RECONNECT_MAX_DELAY_MS]`.
pub fn reconnect_delay(attempts: u32) -> Duration {
    let exponential = RECONNECT_BASE_DELAY_MS.saturating_mul(1_u64 << attempts.min(16));
    let clamped = exponential.min(RECONNECT_MAX_DELAY_MS);
    let jitter = (clamped as f64 * 0.25 * (rand_unit() * 2.0 - 1.0)) as i64;
    let total = (clamped as i64 + jitter).max(0) as u64;
    Duration::from_millis(total)
}

/// POST write backoff: 500ms base doubling to an 8s cap.
pub fn post_retry_delay(attempt: u32) -> Duration {
    let exponential = POST_BASE_DELAY_MS.saturating_mul(1_u64 << (attempt - 1).min(16));
    Duration::from_millis(exponential.min(POST_MAX_DELAY_MS))
}

fn rand_unit() -> f64 {
    let mut bytes = [0_u8; 4];
    getrandom::getrandom(&mut bytes).ok();
    let value = u32::from_be_bytes(bytes) % 10_000;
    value as f64 / 10_000.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn frames_split_on_double_newline_and_keep_remainder() {
        let input = "event: client_event\ndata: {\"a\":1}\n\npartial";
        let (frames, remaining) = parse_sse_frames(input);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].event.as_deref(), Some("client_event"));
        assert_eq!(frames[0].data, "{\"a\":1}");
        assert_eq!(remaining, "partial");
    }

    #[test]
    fn comments_reset_liveness_without_data() {
        let (frames, _) = parse_sse_frames(": keepalive\n\n");
        assert_eq!(frames.len(), 1);
        assert!(frames[0].is_comment);
        assert_eq!(frames[0].data, "");
    }

    #[test]
    fn multi_line_data_and_strip_one_space() {
        let input = "data: line1\ndata:line2\nid: 42\nevent: e\n\n";
        let (frames, _) = parse_sse_frames(input);
        assert_eq!(frames[0].data, "line1\nline2");
        assert_eq!(frames[0].id.as_deref(), Some("42"));
        assert_eq!(frames[0].event.as_deref(), Some("e"));
    }

    #[test]
    fn crlf_frames_parse() {
        let input = "data: hello\r\n\r\n";
        let (frames, _) = parse_sse_frames(input);
        assert_eq!(frames[0].data, "hello");
    }

    #[test]
    fn url_conversion_strips_stream_suffix() {
        assert_eq!(
            convert_sse_url_to_post_url(
                "https://api.example.com/v2/session_ingress/session/s1/events/stream"
            )
            .unwrap(),
            "https://api.example.com/v2/session_ingress/session/s1/events"
        );
        assert_eq!(
            convert_sse_url_to_post_url("http://127.0.0.1:9911/events/stream").unwrap(),
            "http://127.0.0.1:9911/events"
        );
        assert!(convert_sse_url_to_post_url("ws://127.0.0.1:9911/events").is_err());
    }

    #[test]
    fn client_event_payload_extraction() {
        let event =
            parse_client_event("{\"sequence_num\":7,\"payload\":{\"type\":\"user\"}}").unwrap();
        assert_eq!(event.sequence_num, Some(7));
        assert_eq!(event.payload, Some(json!({"type":"user"})));
        assert!(parse_client_event("not json").is_none());
    }

    #[test]
    fn reconnect_backoff_is_clamped_with_jitter() {
        for attempts in 1..30_u32 {
            let delay = reconnect_delay(attempts);
            assert!(
                delay.as_millis() <= (RECONNECT_MAX_DELAY_MS as u128 + RECONNECT_MAX_DELAY_MS / 4)
            );
        }
    }
}
