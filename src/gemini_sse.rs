//! Gemini generateContent-compatible SSE transport; no vendor login or telemetry.
use crate::{
    protocol::{FunctionCall, Message, ModelResponse, TokenUsage, ToolCall},
    provider::parse_gemini_usage,
};
use anyhow::{bail, Context, Result};
use futures::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Default)]
struct Accumulator {
    text: String,
    calls: Vec<ToolCall>,
    usage: Option<TokenUsage>,
    finish_reason: Option<String>,
    saw_done: bool,
}
impl Accumulator {
    fn consume(&mut self, v: &Value, deltas: &UnboundedSender<String>) -> Result<()> {
        if let Some(error) = v.get("error") {
            bail!("Streamed provider error: {error}")
        }
        if let Some(usage) = parse_gemini_usage(v) {
            self.usage = Some(usage);
        }
        let Some(candidate) = v
            .get("candidates")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
        else {
            return Ok(());
        };
        if let Some(finish) = candidate.get("finishReason").and_then(Value::as_str) {
            self.finish_reason = Some(finish.into());
        }
        if let Some(parts) = candidate
            .pointer("/content/parts")
            .and_then(Value::as_array)
        {
            let mut text_parts = Vec::new();
            for part in parts {
                if let Some(text) = part.get("text").and_then(Value::as_str) {
                    text_parts.push(text);
                }
                if let Some(call) = part.get("functionCall") {
                    let name = call
                        .get("name")
                        .and_then(Value::as_str)
                        .context("Gemini streamed tool call without name")?;
                    self.calls.push(ToolCall {
                        id: uuid::Uuid::new_v4().to_string(),
                        kind: "function".into(),
                        function: FunctionCall {
                            name: name.into(),
                            arguments: serde_json::to_string(
                                call.get("args").unwrap_or(&json!({})),
                            )?,
                        },
                    });
                }
            }
            if !text_parts.is_empty() {
                let text = text_parts.join("\n");
                self.text.push_str(&text);
                let _ = deltas.send(text);
            }
        }
        Ok(())
    }
    fn complete(self) -> ModelResponse {
        ModelResponse {
            message: Message {
                role: "assistant".into(),
                content: if self.text.is_empty() {
                    None
                } else {
                    Some(self.text)
                },
                content_blocks: None,
                tool_calls: if self.calls.is_empty() {
                    None
                } else {
                    Some(self.calls)
                },
                tool_call_id: None,
            },
            finish_reason: self.finish_reason,
            usage: self.usage,
        }
    }
}
fn frame_boundary(buffer: &[u8]) -> Option<(usize, usize)> {
    let lf = buffer.windows(2).position(|v| v == b"\n\n");
    let crlf = buffer.windows(4).position(|v| v == b"\r\n\r\n");
    match (lf, crlf) {
        (Some(l), Some(c)) if l <= c => Some((l, 2)),
        (Some(_), Some(c)) => Some((c, 4)),
        (Some(l), None) => Some((l, 2)),
        (None, Some(c)) => Some((c, 4)),
        (None, None) => None,
    }
}
fn frame(data: &[u8], assembler: &mut Accumulator, deltas: &UnboundedSender<String>) -> Result<()> {
    let text = std::str::from_utf8(data).context("Invalid UTF-8 Gemini SSE frame")?;
    let payload = text
        .lines()
        .filter_map(|line| line.strip_prefix("data:").map(str::trim_start))
        .collect::<Vec<_>>()
        .join("\n");
    if payload.is_empty() {
        return Ok(());
    }
    if payload == "[DONE]" {
        assembler.saw_done = true;
        return Ok(());
    }
    let value: Value = serde_json::from_str(&payload).context("Invalid JSON Gemini SSE event")?;
    assembler.consume(&value, deltas)
}
pub async fn complete(
    client: &Client,
    base_url: &str,
    key: &str,
    model: &str,
    body: &Value,
    deltas: UnboundedSender<String>,
) -> Result<ModelResponse> {
    let model = model.strip_prefix("models/").unwrap_or(model);
    let url = format!(
        "{}/models/{}:streamGenerateContent?alt=sse",
        base_url.trim_end_matches('/'),
        model
    );
    let response = client.post(url).header("x-goog-api-key", key).json(body);
    let response = crate::provider::send_model_request(response)
        .await
        .context("Gemini stream request failed")?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await?;
        bail!(
            "Gemini stream HTTP {status}: {}",
            body.chars().take(500).collect::<String>()
        );
    }
    let mut stream = response.bytes_stream();
    let mut assembler = Accumulator::default();
    let mut pending = Vec::<u8>::new();
    while let Some(bytes) = stream.next().await {
        let chunk = bytes.context("Gemini stream socket failed")?;
        pending.extend_from_slice(&chunk);
        if pending.len() > 4_194_304 {
            bail!("Gemini SSE frame exceeds 4 MiB")
        }
        while let Some((end, terminator_len)) = frame_boundary(&pending) {
            let event = pending.drain(..end + terminator_len).collect::<Vec<_>>();
            frame(&event[..end], &mut assembler, &deltas)?;
        }
    }
    if !pending.is_empty() {
        frame(&pending, &mut assembler, &deltas)?;
    }
    // A broken connection may leave a plausible-looking partial answer or
    // function call. Original stream handling requires a terminal stop reason;
    // Gemini also permits an explicit SSE [DONE] marker.
    if assembler.finish_reason.is_none() && !assembler.saw_done {
        bail!("Gemini stream ended before a completion marker");
    }
    Ok(assembler.complete())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn event_aggregation_preserves_deltas_calls_and_usage() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut agg = Accumulator::default();
        frame(
            br#"data: {"candidates":[{"content":{"parts":[{"text":"Hello "}]}}]}"#,
            &mut agg,
            &tx,
        )
        .unwrap();
        frame(br#"data: {"candidates":[{"content":{"parts":[{"text":"world"},{"functionCall":{"name":"read_file","args":{"path":"a.txt"}}}]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":10,"candidatesTokenCount":4,"totalTokenCount":14}}"#,
            &mut agg,&tx).unwrap();
        assert_eq!(rx.try_recv().unwrap(), "Hello ");
        assert_eq!(rx.try_recv().unwrap(), "world");
        let result = agg.complete();
        assert_eq!(result.message.content.as_deref(), Some("Hello world"));
        assert_eq!(
            result.message.tool_calls.as_ref().unwrap()[0].function.name,
            "read_file"
        );
        assert_eq!(result.usage.unwrap().total_tokens, 14);
        assert_eq!(result.finish_reason.as_deref(), Some("STOP"));
    }
    #[test]
    fn frame_delimiters_and_malformed_payload() {
        assert_eq!(frame_boundary(b"a\n\nb"), Some((1, 2)));
        assert_eq!(frame_boundary(b"a\r\n\r\nb"), Some((1, 4)));
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        assert!(frame(b"data: {", &mut Accumulator::default(), &tx).is_err());
        assert!(frame(b"data: [DONE]", &mut Accumulator::default(), &tx).is_ok());
    }
    #[tokio::test]
    async fn gemini_stream_retries_rate_limit_before_first_delta() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            for attempt in 0..2 {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut data = [0u8; 4096];
                socket.read_exact(&mut data[..1]).await.unwrap();
                let (status, body) = if attempt == 0 {
                    (
                        "429 Too Many Requests",
                        r#"{"error":"rate limited"}"#.to_string(),
                    )
                } else {
                    ("200 OK",concat!(
                        "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Recovered\"}]},\"finishReason\":\"STOP\"}]}\n\n"
                    ).to_string())
                };
                socket.write_all(format!(
                    "HTTP/1.1 {status}\r\nRetry-After: 0\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()).as_bytes()).await.unwrap();
            }
        });
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let response = complete(
            &Client::new(),
            &format!("http://{address}/v1beta"),
            "test-key",
            "test-model",
            &json!({"contents":[]}),
            tx,
        )
        .await
        .unwrap();
        server.await.unwrap();
        assert_eq!(response.message.content.as_deref(), Some("Recovered"));
        assert_eq!(rx.try_recv().unwrap(), "Recovered");
        assert!(rx.try_recv().is_err());
    }
}
