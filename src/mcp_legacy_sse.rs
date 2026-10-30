//! Legacy (2024-11-05) MCP HTTP+SSE transport. The original 2.1.88
//! `mcp add --transport sse` uses a long-lived GET for server->client JSON-RPC
//! and a negotiated POST endpoint for client->server messages.
use crate::{compat_helpers, config::McpServerConfig};
use anyhow::{bail, Context, Result};
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue, ACCEPT, CONTENT_TYPE},
    Client, Response, Url,
};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

const MAX_EVENT_BYTES: usize = 2 * 1024 * 1024;
const MAX_RESOURCE_EVENTS: usize = 256;
const MAX_RESOURCE_BYTES: usize = 8 * 1024 * 1024;

pub struct LegacySseClient {
    name: String,
    client: Client,
    endpoint: Url,
    stream: Response,
    headers: HeaderMap,
    buffered: Vec<u8>,
    id: u64,
    root: PathBuf,
    pub server_info: Option<Value>,
    pub capabilities: Option<Value>,
    pub instructions: Option<String>,
    pub tools_changed: bool,
    pub resource_updates: Vec<Value>,
    resource_bytes: usize,
}

fn same_origin(a: &Url, b: &Url) -> bool {
    a.scheme() == b.scheme()
        && a.host_str() == b.host_str()
        && a.port_or_known_default() == b.port_or_known_default()
}

fn endpoint_url(base: &Url, value: &str) -> Result<Url> {
    let endpoint = base.join(value).context("Invalid MCP SSE endpoint")?;
    if !same_origin(base, &endpoint)
        || !matches!(endpoint.scheme(), "http" | "https")
        || !endpoint.username().is_empty()
        || endpoint.password().is_some()
    {
        bail!("MCP SSE endpoint must use the configured server origin without credentials")
    }
    Ok(endpoint)
}

fn parse_frame(frame: &str) -> Result<(String, String)> {
    let mut kind = String::new();
    let mut data = Vec::new();
    for line in frame.lines() {
        let line = line.trim_end_matches('\r');
        if let Some(value) = line.strip_prefix("event:") {
            kind = value.trim().to_owned();
        } else if let Some(value) = line.strip_prefix("data:") {
            data.push(value.strip_prefix(' ').unwrap_or(value));
        }
    }
    Ok((kind, data.join("\n")))
}

impl LegacySseClient {
    pub fn drain_resource_updates(&mut self) -> Vec<Value> {
        self.resource_bytes = 0;
        std::mem::take(&mut self.resource_updates)
    }

    async fn next_frame(&mut self, limit: usize) -> Result<(String, String)> {
        loop {
            if let Some((end, length)) = self
                .buffered
                .windows(2)
                .position(|bytes| bytes == b"\n\n")
                .map(|pos| (pos, 2))
                .or_else(|| {
                    self.buffered
                        .windows(4)
                        .position(|bytes| bytes == b"\r\n\r\n")
                        .map(|pos| (pos, 4))
                })
            {
                let raw = self.buffered.drain(..end + length).collect::<Vec<_>>();
                let frame = std::str::from_utf8(&raw[..end]).context("MCP SSE invalid UTF-8")?;
                let parsed = parse_frame(frame)?;
                if parsed.1.is_empty() {
                    continue;
                }
                return Ok(parsed);
            }
            if self.buffered.len() > limit {
                bail!("MCP SSE frame exceeds {limit} bytes");
            }
            let chunk = self
                .stream
                .chunk()
                .await
                .context("MCP SSE connection read failed")?
                .context("MCP SSE connection closed")?;
            if self
                .buffered
                .len()
                .checked_add(chunk.len())
                .is_none_or(|n| n > limit)
            {
                bail!("MCP SSE frame exceeds {limit} bytes");
            }
            self.buffered.extend_from_slice(&chunk);
        }
    }

    async fn send(&self, value: &Value) -> Result<()> {
        let response = self
            .client
            .post(self.endpoint.clone())
            .headers(self.headers.clone())
            .header(CONTENT_TYPE, "application/json")
            .json(value)
            .send()
            .await
            .context("MCP SSE POST failed")?;
        if !response.status().is_success() {
            bail!("MCP SSE POST rejected: {}", response.status());
        }
        Ok(())
    }

    fn handle_notification(&mut self, message: Value) -> Result<()> {
        match message.get("method").and_then(Value::as_str) {
            Some("notifications/tools/list_changed") => self.tools_changed = true,
            Some("notifications/resources/list_changed" | "notifications/resources/updated") => {
                let length = serde_json::to_vec(&message)?.len();
                if self.resource_updates.len() >= MAX_RESOURCE_EVENTS
                    || self
                        .resource_bytes
                        .checked_add(length)
                        .is_none_or(|n| n > MAX_RESOURCE_BYTES)
                {
                    bail!("MCP SSE notification queue full");
                }
                self.resource_bytes += length;
                self.resource_updates.push(message);
            }
            _ => {}
        }
        Ok(())
    }

    pub async fn connect_with_root(config: &McpServerConfig, root: &Path) -> Result<Self> {
        let source = config.url.as_deref().context("MCP SSE requires url")?;
        let expanded = compat_helpers::expand_env_vars_in_string(source);
        if !expanded.missing_vars.is_empty() {
            bail!(
                "MCP {} missing SSE URL variables: {}",
                config.id,
                expanded.missing_vars.join(", ")
            );
        }
        let url = Url::parse(&expanded.expanded).context("Invalid MCP SSE URL")?;
        if !matches!(url.scheme(), "http" | "https")
            || !url.username().is_empty()
            || url.password().is_some()
        {
            bail!("MCP SSE URL requires http(s) without embedded credentials");
        }
        let dynamic = crate::mcp_http::load_dynamic_headers(config, &url, root).await;
        let mut merged = std::collections::BTreeMap::new();
        for (name, value) in &config.headers {
            merged.insert(name.to_ascii_lowercase(), value.clone());
        }
        if let Some(dynamic) = dynamic {
            for (name, value) in dynamic {
                merged.insert(name.to_ascii_lowercase(), value);
            }
        }
        crate::config::require_secure_credential_transport(
            url.as_str(),
            config
                .headers_helper
                .as_ref()
                .is_some_and(|value| !value.trim().is_empty())
                || merged
                    .iter()
                    .any(|(name, value)| crate::config::credential_header(name, value)),
        )?;
        let mut headers = HeaderMap::new();
        for (name, value) in merged {
            let name =
                HeaderName::from_bytes(name.as_bytes()).context("Invalid SSE header name")?;
            if matches!(
                name.as_str(),
                "accept" | "content-type" | "host" | "last-event-id"
            ) {
                bail!("MCP SSE reserved header cannot be overridden");
            }
            let expanded = compat_helpers::expand_env_vars_in_string(&value);
            if !expanded.missing_vars.is_empty() {
                bail!(
                    "MCP {} missing header environment variables: {}",
                    config.id,
                    expanded.missing_vars.join(", ")
                );
            }
            headers.insert(
                name,
                HeaderValue::from_str(&expanded.expanded).context("Invalid SSE header value")?,
            );
        }
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let response = client
            .get(url.clone())
            .headers(headers.clone())
            .header(ACCEPT, "text/event-stream")
            .send()
            .await
            .context("MCP SSE handshake failed")?;
        if !response.status().is_success() {
            bail!("MCP SSE handshake returned {}", response.status());
        }
        if !response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|x| x.to_str().ok())
            .is_some_and(|value| value.starts_with("text/event-stream"))
        {
            bail!("MCP SSE server did not return text/event-stream");
        }
        let mut out = Self {
            name: config.id.clone(),
            client,
            endpoint: url.clone(),
            stream: response,
            headers,
            buffered: Vec::new(),
            id: 1,
            root: root.to_path_buf(),
            server_info: None,
            capabilities: None,
            instructions: None,
            tools_changed: false,
            resource_updates: Vec::new(),
            resource_bytes: 0,
        };
        let (kind, value) =
            tokio::time::timeout(Duration::from_secs(20), out.next_frame(MAX_EVENT_BYTES))
                .await
                .context("MCP SSE endpoint negotiation timed out")??;
        if kind != "endpoint" {
            bail!("MCP SSE must negotiate an endpoint before JSON-RPC");
        }
        out.endpoint = endpoint_url(&url, &value)?;
        let init = tokio::time::timeout(
            Duration::from_secs(30),
            out.request(
                "initialize",
                json!({"protocolVersion":"2024-11-05","capabilities":{"roots":{}},
                "clientInfo":{"name":"atlas-agent","version":env!("CARGO_PKG_VERSION")}}),
            ),
        )
        .await
        .context("MCP SSE initialize timed out")??;
        out.server_info = init.get("serverInfo").cloned();
        out.capabilities = init.get("capabilities").cloned();
        out.instructions = init
            .get("instructions")
            .and_then(Value::as_str)
            .map(str::to_owned);
        out.send(&json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}}))
            .await?;
        Ok(out)
    }

    pub async fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.id;
        self.id = self
            .id
            .checked_add(1)
            .context("MCP SSE request ID overflow")?;
        self.send(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
            .await?;
        let limit = if method == "tools/call" {
            128 * 1024 * 1024
        } else {
            MAX_EVENT_BYTES
        };
        loop {
            let (kind, data) = self.next_frame(limit).await?;
            if kind != "message" && !kind.is_empty() {
                continue;
            }
            let message: Value = serde_json::from_str(&data).context("Invalid MCP SSE JSON-RPC")?;
            if let Some(reply) = crate::mcp::server_request_reply_for_root(&message, &self.root)? {
                self.send(&reply).await?;
                continue;
            }
            if message.get("id").and_then(Value::as_u64) == Some(id) {
                if message.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
                    bail!("Invalid MCP SSE JSON-RPC version");
                }
                if let Some(error) = message.get("error") {
                    bail!("MCP SSE server error: {error}");
                }
                return message
                    .get("result")
                    .cloned()
                    .context("MCP SSE response missing result");
            }
            self.handle_notification(message)?;
        }
    }

    /// The legacy SSE connection may carry notifications between requests.
    pub async fn poll_notifications(&mut self, deadline: Duration) -> Result<usize> {
        let mut count = 0;
        let expires = tokio::time::Instant::now() + deadline;
        loop {
            let remaining = expires.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            let frame = tokio::time::timeout(remaining, self.next_frame(MAX_EVENT_BYTES)).await;
            let (kind, data) = match frame {
                Ok(value) => value?,
                Err(_) => break,
            };
            if kind != "message" && !kind.is_empty() {
                continue;
            }
            let message: Value =
                serde_json::from_str(&data).context("Invalid MCP SSE notification")?;
            if let Some(reply) = crate::mcp::server_request_reply_for_root(&message, &self.root)? {
                self.send(&reply).await?;
            } else {
                self.handle_notification(message)?;
                count += 1;
                break;
            }
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn endpoint_url_requires_same_origin_and_rejects_credentials() {
        let base = Url::parse("https://example.org/sse").unwrap();
        assert_eq!(
            endpoint_url(&base, "/messages?session=one").unwrap().path(),
            "/messages"
        );
        assert!(endpoint_url(&base, "https://other.org/messages").is_err());
        assert!(endpoint_url(&base, "http://example.org/messages").is_err());
        assert!(endpoint_url(&base, "https://foo:password@example.org/messages").is_err());
    }
    #[test]
    fn sse_frame_handles_crlf_and_multiline_data() {
        assert_eq!(
            parse_frame("event: endpoint\r\ndata: /messages\r\n").unwrap(),
            ("endpoint".to_owned(), "/messages".to_owned())
        );
        assert_eq!(
            parse_frame("event: message\ndata: {\"a\":\ndata: 3}").unwrap(),
            ("message".to_owned(), "{\"a\":\n3}".to_owned())
        );
    }
}
