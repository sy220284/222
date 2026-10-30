//! Generic MCP Streamable HTTP transport (2025-03-26).
//! Requests are individually timed; session headers are never logged.
use crate::{compat_helpers, config::McpServerConfig};
use anyhow::{bail, Context, Result};
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue, ACCEPT, CONTENT_TYPE},
    Client, Url,
};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};
pub struct HttpClient {
    name: String,
    client: Client,
    url: Url,
    headers: HeaderMap,
    session: Option<HeaderValue>,
    id: u64,
    pub server_info: Option<Value>,
    pub capabilities: Option<Value>,
    pub instructions: Option<String>,
    pub tools_changed: bool,
    pub resource_updates: Vec<Value>,
    root: PathBuf,
    last_event_id: Option<HeaderValue>,
}
/// Execute a configured credential helper once when opening a remote MCP connection.
/// A failed helper must not disable the server: the static headers remain usable.
async fn load_dynamic_headers(
    config: &McpServerConfig,
    url: &Url,
    root: &Path,
) -> Option<std::collections::HashMap<String, String>> {
    let helper = config.headers_helper.as_deref()?.trim();
    if helper.is_empty() {
        return None;
    }
    #[cfg(windows)]
    let mut command = {
        let mut command = tokio::process::Command::new("cmd");
        command.arg("/C").arg(helper);
        command
    };
    #[cfg(not(windows))]
    let mut command = {
        let mut command = tokio::process::Command::new("sh");
        command.arg("-c").arg(helper);
        command
    };
    command
        .current_dir(root)
        .env("ATLAS_MCP_SERVER_NAME", &config.id)
        .env("ATLAS_MCP_SERVER_URL", url.as_str());
    let output = crate::exec_file::execute(
        command,
        crate::exec_file::Options {
            timeout: Duration::from_secs(10),
            max_buffer: 1_000_000,
            preserve_output_on_error: false,
            ..Default::default()
        },
    )
    .await;
    if !output.success() || output.stdout.is_empty() {
        return None;
    }
    // Parsing into a string map rejects arrays, nulls, and non-string values.
    serde_json::from_str(&output.stdout).ok()
}

// MCP tools may return large results, but a malformed peer must never force
// unbounded in-memory buffering of JSON or an unterminated SSE frame.
const MAX_MCP_CONTROL_BYTES: usize = 2 * 1024 * 1024;
const MAX_MCP_TOOL_BYTES: usize = 128 * 1024 * 1024;

async fn bounded_json_response(mut response: reqwest::Response, limit: usize) -> Result<Value> {
    if response.content_length().is_some_and(|length| length > limit as u64) {
        bail!("MCP JSON response exceeds {limit} byte limit");
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if body.len().checked_add(chunk.len()).is_none_or(|size| size > limit) {
            bail!("MCP JSON response exceeds {limit} byte limit");
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).context("Invalid MCP JSON response")
}

impl HttpClient {
    pub async fn connect(config: &McpServerConfig) -> Result<Self> {
        let root = std::env::current_dir().context("MCP workspace directory unavailable")?;
        Self::connect_with_root(config, &root).await
    }
    pub async fn connect_with_root(config: &McpServerConfig, root: &Path) -> Result<Self> {
        let url = config.url.as_deref().context("HTTP MCP requires url")?;
        let expanded = compat_helpers::expand_env_vars_in_string(url);
        if !expanded.missing_vars.is_empty() {
            bail!(
                "MCP {} missing URL environment variables: {}",
                config.id,
                expanded.missing_vars.join(", ")
            );
        }
        let url = Url::parse(&expanded.expanded).context("Invalid MCP URL")?;
        if !matches!(url.scheme(), "http" | "https") {
            bail!("MCP HTTP requires http(s)")
        }
        if !url.username().is_empty() || url.password().is_some() {
            bail!("MCP URL must not contain credentials");
        }
        let dynamic_headers = load_dynamic_headers(config, &url, root)
            .await
            .unwrap_or_default();
        // Merge *before* expansion and validation: an overridden static
        // credential must not fail the connection when its variable is unset.
        // HTTP header names are case-insensitive, including helper overrides.
        let mut merged = std::collections::BTreeMap::new();
        for (key, value) in &config.headers {
            merged.insert(key.to_ascii_lowercase(), value.as_str());
        }
        for (key, value) in &dynamic_headers {
            merged.insert(key.to_ascii_lowercase(), value.as_str());
        }
        let mut headers = HeaderMap::new();
        for (key, value) in merged {
            let key = HeaderName::from_str(&key).context("Invalid MCP header name")?;
            if matches!(
                key.as_str(),
                "accept"
                    | "content-type"
                    | "mcp-session-id"
                    | "mcp-protocol-version"
                    | "host"
                    | "last-event-id"
            ) {
                bail!("MCP reserved header cannot be overridden: {key}");
            }
            let expanded = compat_helpers::expand_env_vars_in_string(value);
            if !expanded.missing_vars.is_empty() {
                bail!(
                    "MCP {} missing header environment variables: {}",
                    config.id,
                    expanded.missing_vars.join(", ")
                );
            }
            headers.insert(
                key,
                HeaderValue::from_str(&expanded.expanded).context("Invalid MCP header value")?,
            );
        }
        let client = Client::builder()
            // Only bound connection establishment here. Request-specific
            // deadlines are enforced by the MCP manager; tools/call can
            // legitimately run far longer than discovery or initialization.
            .connect_timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let mut http = Self {
            name: config.id.clone(),
            client,
            url,
            headers,
            session: None,
            id: 1,
            server_info: None,
            capabilities: None,
            instructions: None,
            tools_changed: false,
            resource_updates: Vec::new(),
            root: root.to_path_buf(),
            last_event_id: None,
        };
        let initial = tokio::time::timeout(
            Duration::from_secs(30),
            http.request(
                "initialize",
                json!({
                    "protocolVersion":"2025-03-26","capabilities":{"roots":{},"elicitation":{}},
                    "clientInfo":{"name":"atlas-agent","version":env!("CARGO_PKG_VERSION")}
                }),
            ),
        )
        .await
        .context("MCP HTTP initialization timed out")??;
        http.server_info = initial.get("serverInfo").cloned();
        http.capabilities = initial.get("capabilities").cloned();
        http.instructions = initial
            .get("instructions")
            .and_then(Value::as_str)
            .filter(|text| !text.trim().is_empty())
            .map(str::to_owned);
        tokio::time::timeout(
            Duration::from_secs(30),
            http.notify("notifications/initialized", json!({})),
        )
        .await
        .context("MCP HTTP initialized notification timed out")??;
        Ok(http)
    }
    fn request_headers(&self) -> HeaderMap {
        let mut headers = self.headers.clone();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/json, text/event-stream"),
        );
        headers.insert(
            HeaderName::from_static("mcp-protocol-version"),
            HeaderValue::from_static("2025-03-26"),
        );
        if let Some(session) = &self.session {
            headers.insert(HeaderName::from_static("mcp-session-id"), session.clone());
        }
        headers
    }
    async fn reply_to_server(&self, response: &Value) -> Result<()> {
        // Keep the SSE response stream open while replying via a separate POST.
        let reply = self
            .client
            .post(self.url.clone())
            .headers(self.request_headers())
            .json(response)
            .send()
            .await
            .with_context(|| format!("HTTP MCP {} server-request reply failed", self.name))?;
        if !reply.status().is_success() {
            bail!(
                "HTTP MCP {} server-request reply rejected: {}",
                self.name,
                reply.status()
            );
        }
        Ok(())
    }
    async fn post(&mut self, request: &Value, expect_response: bool) -> Result<Option<Value>> {
        let headers = self.request_headers();
        let is_tool_call = request.get("method").and_then(Value::as_str) == Some("tools/call");
        let response_limit = if is_tool_call { MAX_MCP_TOOL_BYTES } else { MAX_MCP_CONTROL_BYTES };
        let mut reply = self
            .client
            .post(self.url.clone())
            .headers(headers)
            .json(request)
            .send()
            .await
            .with_context(|| format!("HTTP MCP {} request failed", self.name))?;
        if reply.status().is_redirection() {
            bail!("MCP redirect blocked")
        }
        if reply.status() == reqwest::StatusCode::NOT_FOUND && self.session.is_some() {
            bail!("MCP session expired; reconnect required");
        }
        // A rejected authorization must invalidate this transport. The
        // manager creates a new connection using the configured dynamic
        // headers helper on the next turn, without replaying the tool call.
        if reply.status() == reqwest::StatusCode::UNAUTHORIZED {
            bail!("MCP authentication expired; reconnect required");
        }
        // RFC 6750: a 403 insufficient_scope requires renewed user
        // authorization. Refreshing the same credential cannot add scope.
        if reply.status() == reqwest::StatusCode::FORBIDDEN
            && reply
                .headers()
                .get("www-authenticate")
                .and_then(|header| header.to_str().ok())
                .is_some_and(|header| header.contains("insufficient_scope"))
        {
            bail!("MCP authorization needs additional scope; user reauthorization required");
        }
        if !reply.status().is_success() {
            bail!("MCP {} server responded {}", self.name, reply.status())
        }
        if self.session.is_none() {
            if let Some(id) = reply.headers().get("mcp-session-id") {
                if id.as_bytes().is_empty() || id.as_bytes().len() > 512 {
                    bail!("MCP session ID has invalid length")
                }
                self.session = Some(id.clone());
            }
        }
        if !expect_response {
            return Ok(None);
        }
        if reply.status() == reqwest::StatusCode::ACCEPTED {
            bail!("MCP request received 202 without JSON-RPC response");
        }
        let type_header = reply
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_ascii_lowercase();
        if type_header.starts_with("text/event-stream") {
            // Streamable HTTP servers may keep this response open after the
            // requested result. Never wait for EOF: that deadlocks tools/call
            // and hides notifications delivered ahead of the result.
            let id = request.get("id").context("MCP request is missing id")?;
            let mut pending = Vec::new();
            let mut received = 0usize;
            while let Some(chunk) = reply.chunk().await? {
                received = received
                    .checked_add(chunk.len())
                    .context("MCP SSE response length overflow")?;
                if received > response_limit {
                    bail!("MCP HTTP response exceeds {response_limit} byte limit")
                }
                pending.extend_from_slice(&chunk);
                while let Some((end, separator_len)) = sse_event_boundary(&pending) {
                    let frame =
                        std::str::from_utf8(&pending[..end]).context("Invalid MCP SSE UTF-8")?;
                    for event in parse_sse(frame)? {
                        if let Some(response) =
                            crate::mcp::server_request_reply_for_root(&event, &self.root)?
                        {
                            self.reply_to_server(&response).await?;
                            continue;
                        }
                        let kind = event.get("method").and_then(Value::as_str);
                        if kind == Some("notifications/tools/list_changed") {
                            self.tools_changed = true;
                        }
                        if matches!(
                            kind,
                            Some(
                                "notifications/resources/updated"
                                    | "notifications/resources/list_changed"
                            )
                        ) {
                            self.resource_updates.push(event.clone());
                        }
                        if event.get("id") == Some(id) {
                            return Ok(Some(event));
                        }
                    }
                    pending.drain(..end + separator_len);
                }
            }
            bail!("MCP SSE stream ended without matching RPC response")
        }
        if type_header.starts_with("application/json") {
            return Ok(Some(bounded_json_response(reply, response_limit).await?));
        }
        bail!("MCP HTTP response has unsupported content type")
    }
    /// Pull independent server-to-client events on a Streamable HTTP GET.
    /// This explicit, bounded long poll lets SDK callers receive notifications
    /// that are not attached to a tools/call response. Event IDs are retained
    /// for resumption; server-initiated requests receive a JSON-RPC POST reply.
    pub async fn poll_notifications(&mut self, max_wait: Duration) -> Result<usize> {
        if max_wait.is_zero() {
            return Ok(0);
        }
        let mut headers = self.request_headers();
        headers.insert(ACCEPT, HeaderValue::from_static("text/event-stream"));
        headers.remove(CONTENT_TYPE);
        if let Some(id) = &self.last_event_id {
            headers.insert(HeaderName::from_static("last-event-id"), id.clone());
        }
        let deadline = tokio::time::Instant::now() + max_wait;
        let mut response = tokio::time::timeout(
            max_wait,
            self.client.get(self.url.clone()).headers(headers).send(),
        )
        .await
        .context("MCP GET notification request timed out")??;
        if response.status() == reqwest::StatusCode::METHOD_NOT_ALLOWED {
            return Ok(0); // optional per MCP Streamable HTTP
        }
        if response.status() == reqwest::StatusCode::NOT_FOUND && self.session.is_some() {
            bail!("MCP session expired; reconnect required")
        }
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            bail!("MCP authentication expired; reconnect required")
        }
        if !response.status().is_success() {
            bail!("MCP GET notification request failed: {}", response.status());
        }
        if !response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .starts_with("text/event-stream")
        {
            bail!("MCP GET notifications require text/event-stream")
        }
        let mut pending = Vec::new();
        let mut total = 0usize;
        let mut count = 0usize;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            let next = match tokio::time::timeout(remaining, response.chunk()).await {
                Ok(value) => value?,
                Err(_) => break,
            };
            let Some(chunk) = next else { break };
            total += chunk.len();
            if total > 2 * 1024 * 1024 {
                bail!("MCP notification stream exceeds 2 MB")
            }
            pending.extend_from_slice(&chunk);
            while let Some((end, separator_len)) = sse_event_boundary(&pending) {
                let frame = std::str::from_utf8(&pending[..end])
                    .context("Invalid MCP notification SSE UTF-8")?;
                let mut event_id = None;
                for line in frame.lines() {
                    if let Some(raw) = line.trim_end_matches('\r').strip_prefix("id:") {
                        let raw = raw.trim_start();
                        if raw.len() > 512 || raw.contains('\0') {
                            bail!("Invalid MCP SSE event ID")
                        }
                        event_id = Some(
                            HeaderValue::from_str(raw)
                                .context("Invalid MCP SSE event ID header")?,
                        );
                    }
                }
                for event in parse_sse(frame)? {
                    if let Some(reply) =
                        crate::mcp::server_request_reply_for_root(&event, &self.root)?
                    {
                        self.reply_to_server(&reply).await?;
                    } else {
                        match event.get("method").and_then(Value::as_str) {
                            Some("notifications/tools/list_changed") => {
                                self.tools_changed = true;
                                count += 1;
                            }
                            Some(
                                "notifications/resources/updated"
                                | "notifications/resources/list_changed",
                            ) => {
                                self.resource_updates.push(event);
                                count += 1;
                            }
                            _ => {}
                        }
                    }
                }
                if let Some(id) = event_id {
                    self.last_event_id = Some(id);
                }
                pending.drain(..end + separator_len);
            }
            // Finish once a complete event has been observed instead of
            // blocking until the server drops its long-lived GET stream.
            if count > 0 {
                break;
            }
        }
        Ok(count)
    }

    pub async fn notify(&mut self, method: &str, params: Value) -> Result<()> {
        self.post(
            &json!({"jsonrpc":"2.0","method":method,"params":params}),
            false,
        )
        .await?;
        Ok(())
    }
    pub async fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.id;
        self.id = self.id.checked_add(1).context("MCP ID overflow")?;
        let response = self
            .post(
                &json!({"jsonrpc":"2.0","id":id,
            "method":method,"params":params}),
                true,
            )
            .await?
            .context("MCP response was empty")?;
        if response.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
            || response.get("id").and_then(Value::as_u64) != Some(id)
        {
            bail!("MCP JSON-RPC response version or ID mismatch");
        }
        if let Some(error) = response.get("error") {
            bail!("MCP server error: {error}")
        }
        response
            .get("result")
            .cloned()
            .context("MCP response missing result")
    }
}
// Locate complete SSE events without decoding incomplete UTF-8 chunks.
// CRLF can be split across HTTP chunks, so only complete separators count.
fn sse_event_boundary(bytes: &[u8]) -> Option<(usize, usize)> {
    for index in 0..bytes.len() {
        if bytes[index..].starts_with(b"\r\n\r\n") {
            return Some((index, 4));
        }
        if bytes[index..].starts_with(b"\n\n") {
            return Some((index, 2));
        }
    }
    None
}
fn parse_sse(text: &str) -> Result<Vec<Value>> {
    let mut results = Vec::new();
    let mut data = Vec::new();
    let mut kind = String::new();
    for line in text.lines().chain(std::iter::once("")) {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            if !data.is_empty() && (kind.is_empty() || kind == "message") {
                results.push(
                    serde_json::from_str(&data.join("\n"))
                        .context("Invalid JSON in MCP SSE event")?,
                );
            }
            data.clear();
            kind.clear();
            continue;
        }
        if let Some(value) = line.strip_prefix("data:") {
            data.push(value.strip_prefix(' ').unwrap_or(value).to_owned())
        } else if let Some(value) = line.strip_prefix("event:") {
            kind = value.trim().to_owned()
        }
    }
    Ok(results)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    #[tokio::test]
    async fn oversized_json_tool_result_is_bounded_before_body_read() {
        use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpListener};
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0u8; 2048];
            let _ = socket.read(&mut request).await.unwrap();
            socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 4096\r\nConnection: close\r\n\r\n").await.unwrap();
        });
        let response = reqwest::Client::new().get(format!("http://{address}")).send().await.unwrap();
        let err = bounded_json_response(response, 32).await.unwrap_err();
        assert!(err.to_string().contains("exceeds 32 byte limit"));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn streamed_json_tool_result_is_bounded_without_content_length() {
        use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpListener};
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0u8; 2048];
            let _ = socket.read(&mut request).await.unwrap();
            let payload = format!("{{\"message\":\"{}\"}}", "x".repeat(1000));
            let response = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:X}\r\n{}\r\n0\r\n\r\n", payload.len(), payload);
            socket.write_all(response.as_bytes()).await.unwrap();
        });
        let response = reqwest::Client::new().get(format!("http://{address}")).send().await.unwrap();
        let err = bounded_json_response(response, 64).await.unwrap_err();
        assert!(err.to_string().contains("exceeds 64 byte limit"));
        server.await.unwrap();
    }

    fn config(url: String) -> McpServerConfig {
        McpServerConfig {
            id: "remote".into(),
            command: String::new(),
            args: Vec::new(),
            env: HashMap::new(),
            transport: Some("http".into()),
            url: Some(url),
            headers: HashMap::new(),
            headers_helper: None,
        }
    }
    #[tokio::test]
    async fn independent_get_notifications_preserve_session_and_resume_event_id() {
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::{TcpListener, TcpStream},
            time::timeout,
        };
        async fn request(socket: &mut TcpStream) -> String {
            let mut bytes = Vec::new();
            let mut buf = [0u8; 4096];
            loop {
                let n = socket.read(&mut buf).await.unwrap();
                if n == 0 {
                    break;
                }
                bytes.extend_from_slice(&buf[..n]);
                if let Some(i) = bytes.windows(4).position(|x| x == b"\r\n\r\n") {
                    let header = String::from_utf8_lossy(&bytes[..i]);
                    let len = header
                        .lines()
                        .find_map(|l| {
                            let (k, v) = l.split_once(':')?;
                            k.eq_ignore_ascii_case("content-length")
                                .then(|| v.trim().parse::<usize>().ok())
                                .flatten()
                        })
                        .unwrap_or(0);
                    if bytes.len() >= i + 4 + len {
                        break;
                    }
                }
            }
            String::from_utf8_lossy(&bytes).into_owned()
        }
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let mut requests = Vec::new();
            for step in 0..4 {
                let (mut socket, _) = listener.accept().await.unwrap();
                let req = request(&mut socket).await;
                requests.push(req.clone());
                let (mime, body) = match step {
                    0 => (
                        "application/json",
                        json!({"jsonrpc":"2.0","id":1,"result":{
                            "serverInfo":{"name":"push","version":"1"},
                            "capabilities":{"tools":{},"resources":{"subscribe":true}}}})
                        .to_string(),
                    ),
                    1 => ("application/json", String::new()),
                    2 => (
                        "text/event-stream",
                        format!(
                            "id: cursor-1\r\nevent: message\r\ndata: {}\r\n\r\n",
                            json!({"jsonrpc":"2.0",
                            "method":"notifications/resources/updated",
                            "params":{"uri":"file:///first","revision":4}})
                        ),
                    ),
                    _ => (
                        "text/event-stream",
                        format!(
                            "id: cursor-2\r\nevent: message\r\ndata: {}\r\n\r\n",
                            json!({"jsonrpc":"2.0",
                            "method":"notifications/tools/list_changed"})
                        ),
                    ),
                };
                let status = if step == 1 { "202 Accepted" } else { "200 OK" };
                let header=format!(
                    "HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\n{}Connection: close\r\n\r\n",
                    body.len(),if step==0{"Mcp-Session-Id: session-push\r\n"}else{""}
                );
                socket.write_all(header.as_bytes()).await.unwrap();
                if !body.is_empty() {
                    let split = body.len() / 2;
                    socket.write_all(&body.as_bytes()[..split]).await.unwrap();
                    socket.write_all(&body.as_bytes()[split..]).await.unwrap();
                }
                socket.shutdown().await.unwrap();
            }
            requests
        });
        let mut client = timeout(
            Duration::from_secs(4),
            HttpClient::connect(&config(format!("http://{address}/mcp"))),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(
            timeout(
                Duration::from_secs(4),
                client.poll_notifications(Duration::from_secs(3))
            )
            .await
            .unwrap()
            .unwrap(),
            1
        );
        assert_eq!(client.resource_updates.len(), 1);
        assert_eq!(client.resource_updates[0]["params"]["revision"], 4);
        assert_eq!(
            timeout(
                Duration::from_secs(4),
                client.poll_notifications(Duration::from_secs(3))
            )
            .await
            .unwrap()
            .unwrap(),
            1
        );
        assert!(client.tools_changed);
        let seen = server.await.unwrap();
        assert!(seen[2].starts_with("GET /mcp HTTP/1.1"));
        assert!(seen[2]
            .to_ascii_lowercase()
            .contains("mcp-session-id: session-push"));
        assert!(seen[2]
            .to_ascii_lowercase()
            .contains("accept: text/event-stream"));
        assert!(seen[3]
            .to_ascii_lowercase()
            .contains("last-event-id: cursor-1"));
    }
    #[tokio::test]
    async fn http_server_requests_complete_before_sse_stream_closes() {
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::{TcpListener, TcpStream},
            sync::oneshot,
            time::timeout,
        };
        async fn read_request(socket: &mut TcpStream) -> String {
            let mut input = Vec::new();
            loop {
                let mut buffer = [0u8; 4096];
                let count = socket.read(&mut buffer).await.unwrap();
                assert!(count > 0, "client closed before request body");
                input.extend_from_slice(&buffer[..count]);
                let Some(end) = input.windows(4).position(|w| w == b"\r\n\r\n") else {
                    continue;
                };
                let header = String::from_utf8_lossy(&input[..end]);
                let length = header
                    .lines()
                    .find_map(|line| {
                        let (key, value) = line.split_once(':')?;
                        if key.eq_ignore_ascii_case("content-length") {
                            value.trim().parse::<usize>().ok()
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0);
                if input.len() >= end + 4 + length {
                    return String::from_utf8_lossy(&input).into_owned();
                }
            }
        }
        async fn frame(socket: &mut TcpStream, event: Value) {
            let data = format!("event: message\r\ndata: {event}\r\n\r\n");
            socket
                .write_all(format!("{:X}\r\n{data}\r\n", data.len()).as_bytes())
                .await
                .unwrap();
        }
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let workspace = tempfile::tempdir().unwrap();
        let expected_root =
            reqwest::Url::from_directory_path(workspace.path().canonicalize().unwrap())
                .unwrap()
                .to_string();
        let (release, finish) = oneshot::channel::<()>();
        let server = tokio::spawn(async move {
            let mut sse: Option<TcpStream> = None;
            for step in 0..5 {
                let (mut socket, _) = listener.accept().await.unwrap();
                let raw = read_request(&mut socket).await;
                let (_, body) = raw.split_once("\r\n\r\n").unwrap();
                let value: Value = serde_json::from_str(body).unwrap();
                let accepted =
                    b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                match step {
                    0 => {
                        assert_eq!(value["method"], "initialize");
                        assert_eq!(value["params"]["capabilities"]["roots"], json!({}));
                        assert_eq!(value["params"]["capabilities"]["elicitation"], json!({}));
                        let result = json!({"jsonrpc":"2.0","id":1,"result":{
                            "capabilities":{"tools":{}}}})
                        .to_string();
                        socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nMcp-Session-Id: test-session\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{result}",result.len()).as_bytes()).await.unwrap();
                    }
                    1 => {
                        assert_eq!(value["method"], "notifications/initialized");
                        socket.write_all(accepted).await.unwrap();
                    }
                    2 => {
                        assert_eq!(value["method"], "tools/list");
                        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: keep-alive\r\n\r\n").await.unwrap();
                        frame(
                            &mut socket,
                            json!({"jsonrpc":"2.0","id":91,
                            "method":"roots/list"}),
                        )
                        .await;
                        sse = Some(socket);
                    }
                    3 => {
                        assert!(raw
                            .to_ascii_lowercase()
                            .contains("mcp-session-id: test-session"));
                        assert_eq!(value["id"], 91);
                        assert_eq!(value["result"]["roots"][0]["uri"], expected_root);
                        socket.write_all(accepted).await.unwrap();
                        frame(
                            sse.as_mut().unwrap(),
                            json!({"jsonrpc":"2.0","id":92,
                            "method":"ping"}),
                        )
                        .await;
                    }
                    4 => {
                        assert_eq!(value["id"], 92);
                        assert_eq!(value["result"], json!({}));
                        socket.write_all(accepted).await.unwrap();
                        frame(
                            sse.as_mut().unwrap(),
                            json!({"jsonrpc":"2.0","id":2,
                            "result":{"tools":[{"name":"ready"}]}}),
                        )
                        .await;
                    }
                    _ => unreachable!(),
                }
            }
            // Keep the original SSE connection open until the client returns.
            let _ = finish.await;
        });
        let mut client = timeout(
            Duration::from_secs(4),
            HttpClient::connect_with_root(
                &config(format!("http://{address}/mcp")),
                workspace.path(),
            ),
        )
        .await
        .unwrap()
        .unwrap();
        let result = timeout(
            Duration::from_secs(4),
            client.request("tools/list", json!({})),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(result["tools"][0]["name"], "ready");
        release.send(()).unwrap();
        server.await.unwrap();
    }

    #[tokio::test]
    async fn http_sse_resource_update_before_rpc_response_survives_and_does_not_wait_for_eof() {
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let mut observed = Vec::new();
            for n in 0..4 {
                let (mut connection, _) = listener.accept().await.unwrap();
                let mut input = Vec::new();
                let mut buf = [0u8; 4096];
                loop {
                    let count = connection.read(&mut buf).await.unwrap();
                    assert!(count > 0);
                    input.extend_from_slice(&buf[..count]);
                    if let Some(split) = input.windows(4).position(|b| b == b"\r\n\r\n") {
                        let header = String::from_utf8_lossy(&input[..split]);
                        let len = header
                            .lines()
                            .find_map(|line| {
                                line.to_ascii_lowercase()
                                    .strip_prefix("content-length:")
                                    .and_then(|value| value.trim().parse::<usize>().ok())
                            })
                            .unwrap_or(0);
                        if input.len() >= split + 4 + len {
                            break;
                        }
                    }
                }
                observed.push(String::from_utf8_lossy(&input).into_owned());
                let (kind, body) = match n {
                    0 => (
                        "application/json",
                        json!({"jsonrpc":"2.0","id":1,
                        "result":{"capabilities":{"resources":{"subscribe":true}}}})
                        .to_string(),
                    ),
                    1 => ("application/json", String::new()),
                    2 => (
                        "text/event-stream",
                        format!(
                            "event: message\r\ndata: {}\r\n\r\n\
                        event: message\r\ndata: {}\r\n\r\n",
                            json!({"jsonrpc":"2.0","method":"notifications/resources/updated",
                            "params":{"uri":"file:///image","revision":3}}),
                            json!({"jsonrpc":"2.0","id":2,"result":{}})
                        ),
                    ),
                    _ => (
                        "application/json",
                        json!({"jsonrpc":"2.0","id":3,
                        "result":{}})
                        .to_string(),
                    ),
                };
                let header=format!("HTTP/1.1 {} OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    if n==1{202}else{200},body.len());
                connection.write_all(header.as_bytes()).await.unwrap();
                if !body.is_empty() {
                    // Deliberately split the JSON payload across TCP reads.
                    let mid = body.len() / 2;
                    connection.write_all(&body.as_bytes()[..mid]).await.unwrap();
                    connection.write_all(&body.as_bytes()[mid..]).await.unwrap();
                }
                connection.shutdown().await.unwrap();
            }
            observed
        });
        let mut http = HttpClient::connect(&config(format!("http://{address}/mcp")))
            .await
            .unwrap();
        assert_eq!(
            http.capabilities.as_ref().unwrap()["resources"]["subscribe"],
            true
        );
        http.request("resources/subscribe", json!({"uri":"file:///image"}))
            .await
            .unwrap();
        assert_eq!(http.resource_updates.len(), 1);
        assert_eq!(http.resource_updates[0]["params"]["revision"], 3);
        assert_eq!(
            http.resource_updates[0]["method"],
            "notifications/resources/updated"
        );
        http.request("resources/unsubscribe", json!({"uri":"file:///image"}))
            .await
            .unwrap();
        let requests = server.await.unwrap();
        assert!(requests[2].contains("resources/subscribe"));
        assert!(requests[3].contains("resources/unsubscribe"));
    }
    #[test]
    fn stream_parser_skips_keepalives_and_nonmessage_events() {
        let stream = ": keepalive\r\nevent: update\r\ndata: {}\r\n\r\n\
            event: message\r\ndata: {\"jsonrpc\":\"2.0\",\"id\":3,\r\n\
            data: \"result\":{}}\r\n\r\n";
        let events = parse_sse(stream).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["id"], 3);
    }
    #[tokio::test]
    async fn rejects_url_credentials_and_reserved_headers_without_network() {
        let mut c = config("http://user:password@127.0.0.1:1/mcp".into());
        assert!(
            format!("{:#}", HttpClient::connect(&c).await.err().unwrap()).contains("credentials")
        );
        c.url = Some("http://127.0.0.1:1/mcp".into());
        c.headers.insert("Mcp-Session-Id".into(), "forged".into());
        assert!(
            format!("{:#}", HttpClient::connect(&c).await.err().unwrap())
                .contains("reserved header")
        );
    }
}
