use crate::protocol::{FunctionCall, Message, ModelRequest, ModelResponse, TokenUsage, ToolCall};
use anyhow::{bail, Context, Result};
use futures::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Default)]
struct Assembler {
    content: String,
    reasoning_content: String,
    calls: BTreeMap<u64, ToolCall>,
    finish_reason: Option<String>,
    usage: Option<TokenUsage>,
    /// Live sink for completed tool blocks; None keeps pure aggregation.
    tool_blocks: Option<UnboundedSender<ToolCall>>,
}

impl Assembler {
    fn consume(&mut self, value: &Value, deltas: &UnboundedSender<String>) -> Result<()> {
        // A provider can report a failure inside an already established 200/SSE
        // stream. Never turn a partial assistant response (or tool call) into
        // a successful turn after an in-band error.
        if let Some(error) = value.get("error").filter(|error| !error.is_null()) {
            let detail = error
                .get("message")
                .and_then(Value::as_str)
                .or_else(|| error.as_str())
                .unwrap_or("provider reported a streaming error");
            bail!(
                "Model stream error: {}",
                detail.chars().take(500).collect::<String>()
            );
        }
        if let Some(usage) = value.get("usage") {
            if usage.is_object() {
                let input = usage
                    .get("prompt_tokens")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                let output = usage
                    .get("completion_tokens")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                self.usage = Some(TokenUsage {
                    input_tokens: input,
                    output_tokens: output,
                    total_tokens: usage
                        .get("total_tokens")
                        .and_then(Value::as_u64)
                        .unwrap_or(input.saturating_add(output)),
                    cache_read_input_tokens: usage
                        .pointer("/prompt_tokens_details/cached_tokens")
                        .or_else(|| usage.get("cache_read_input_tokens"))
                        .and_then(Value::as_u64)
                        .unwrap_or(0),
                    cache_creation_input_tokens: usage
                        .get("cache_creation_input_tokens")
                        .or_else(|| usage.get("cache_write_input_tokens"))
                        .and_then(Value::as_u64)
                        .unwrap_or(0),
                    web_search_requests: usage
                        .pointer("/server_tool_use/web_search_requests")
                        .or_else(|| usage.get("web_search_requests"))
                        .and_then(Value::as_u64)
                        .unwrap_or(0),
                    ..Default::default()
                });
            }
        }
        let Some(choice) = value
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|a| a.first())
        else {
            return Ok(());
        };
        if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
            self.finish_reason = Some(reason.into());
        }
        let delta = choice.get("delta").unwrap_or(&Value::Null);
        // OpenAI-compatible transports can emit a refusal as a dedicated
        // delta field while the final finish_reason is simply "stop".
        // Do not silently drop that field and return an empty success.
        if let Some(refusal) = delta.get("refusal").and_then(Value::as_str) {
            bail!(
                "Model refused the request: {}",
                refusal.chars().take(500).collect::<String>()
            );
        }
        if let Some(reasoning) = delta.get("reasoning_content").and_then(Value::as_str) {
            // Reasoning stays in the model transcript for subsequent tool
            // calls, but is never broadcast as a user-visible text delta.
            self.reasoning_content.push_str(reasoning);
        }
        if let Some(text) = delta.get("content").and_then(Value::as_str) {
            self.content.push_str(text);
            let _ = deltas.send(text.into());
        }
        for part in delta
            .get("tool_calls")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let index = part.get("index").and_then(Value::as_u64).unwrap_or(0);
            // The source API adapter yields tools only at content_block_stop.
            // Chat Completions has no equivalent per-tool stop event: index
            // identifies a call, and its later deltas may interleave with
            // any other call. Aggregate until the transport is complete.
            let entry = self.calls.entry(index).or_insert_with(|| ToolCall {
                id: String::new(),
                kind: "function".into(),
                function: FunctionCall {
                    name: String::new(),
                    arguments: String::new(),
                },
            });
            if let Some(id) = part.get("id").and_then(Value::as_str) {
                entry.id.push_str(id);
            }
            if let Some(name) = part.pointer("/function/name").and_then(Value::as_str) {
                entry.function.name.push_str(name);
            }
            if let Some(arguments) = part.pointer("/function/arguments").and_then(Value::as_str) {
                entry.function.arguments.push_str(arguments);
            }
        }
        Ok(())
    }
    fn finish(mut self) -> Result<ModelResponse> {
        if self.content.is_empty() && self.calls.is_empty() {
            bail!("SSE stream contained no assistant content or tool calls")
        }
        // finish is reached only after a terminal marker. Do not start tools
        // from a response the Agent will reject as truncated or refused.
        let can_execute = self
            .finish_reason
            .as_deref()
            .is_none_or(|reason| matches!(reason, "tool_calls" | "stop"));
        for call in self.calls.values_mut() {
            if call.id.is_empty() {
                call.id = uuid::Uuid::new_v4().to_string();
            }
            if can_execute {
                if let Some(sink) = &self.tool_blocks {
                    let _ = sink.send(call.clone());
                }
            }
        }
        let calls = self.calls.into_values().collect::<Vec<_>>();
        Ok(ModelResponse {
            message: Message {
                role: "assistant".into(),
                content: if self.content.is_empty() {
                    None
                } else {
                    Some(self.content)
                },
                reasoning_content: if self.reasoning_content.is_empty() {
                    None
                } else {
                    Some(self.reasoning_content)
                },
                content_blocks: None,
                tool_calls: if calls.is_empty() { None } else { Some(calls) },
                tool_call_id: None,
            },
            finish_reason: self.finish_reason,
            usage: self.usage,
        })
    }
}
fn boundary(buffer: &[u8]) -> Option<(usize, usize)> {
    for i in 0..buffer.len() {
        if buffer[i..].starts_with(b"\r\n\r\n") {
            return Some((i, 4));
        }
        if buffer[i..].starts_with(b"\n\n") {
            return Some((i, 2));
        }
    }
    None
}
fn consume_frame(
    frame: &[u8],
    assembler: &mut Assembler,
    deltas: &UnboundedSender<String>,
) -> Result<bool> {
    let lines = String::from_utf8_lossy(frame);
    let mut data = Vec::new();
    let mut error_event = false;
    for line in lines.lines() {
        if line
            .strip_prefix("event:")
            .is_some_and(|name| name.trim() == "error")
        {
            error_event = true;
        }
        if let Some(body) = line.strip_prefix("data:") {
            data.push(body.trim_start().to_string());
        }
    }
    if data.is_empty() {
        return Ok(false);
    }
    let payload = data.join("\n");
    if payload.trim() == "[DONE]" {
        return Ok(true);
    }
    let value: Value = serde_json::from_str(&payload).context("Invalid model SSE event")?;
    if error_event {
        let detail = value
            .pointer("/error/message")
            .or_else(|| value.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("provider reported an SSE error event");
        bail!(
            "Model stream error: {}",
            detail.chars().take(500).collect::<String>()
        );
    }
    assembler.consume(&value, deltas)?;
    Ok(false)
}

pub async fn complete(
    client: &Client,
    base_url: &str,
    key: Option<&str>,
    request: &ModelRequest,
    deltas: UnboundedSender<String>,
    response_schema: Option<&Value>,
    tuning: &crate::config::ModelTuning,
) -> Result<ModelResponse> {
    complete_with_tool_blocks(
        client,
        base_url,
        key,
        request,
        deltas,
        response_schema,
        None,
        tuning,
    )
    .await
}

/// Streaming completion with an optional live tool-block sink. Blocks are
/// forwarded in index order only after a successful completion boundary.
/// Chat Completions indices are not content_block_stop events.
#[allow(clippy::too_many_arguments)]
pub async fn complete_with_tool_blocks(
    client: &Client,
    base_url: &str,
    key: Option<&str>,
    request: &ModelRequest,
    deltas: UnboundedSender<String>,
    response_schema: Option<&Value>,
    tool_blocks: Option<UnboundedSender<ToolCall>>,
    tuning: &crate::config::ModelTuning,
) -> Result<ModelResponse> {
    let mut body = json!({"model":request.model,
        "messages":crate::provider::openai_messages(request)?,
        "stream":true,"stream_options":{"include_usage":true}});
    tuning.validate_for_model(&request.model)?;
    tuning.apply_chat_completions(&mut body);
    // Keep structured-output semantics identical for streaming and ordinary
    // Chat Completions. Omitting this silently drops the SDK output contract.
    if let Some(schema) = response_schema {
        body["response_format"] = json!({"type":"json_schema",
            "json_schema":{"name":"structured_output","strict":true,"schema":schema}});
    }
    if !request.tools.is_empty() {
        body["tools"] = json!(request.tools);
        body["tool_choice"] = json!("auto");
    }
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let mut post = client.post(url).json(&body);
    if let Some(key) = key {
        post = post.bearer_auth(key)
    }
    let idle_timeout = crate::provider::stream_idle_timeout();
    let response = tokio::time::timeout(idle_timeout, crate::provider::send_model_request(post))
        .await
        .context("Model stream response headers timed out")?
        .context("Streaming model request failed")?;
    let status = response.status();
    if !status.is_success() {
        bail!(
            "Streaming model HTTP {status}: {}",
            response
                .text()
                .await?
                .chars()
                .take(1000)
                .collect::<String>()
        )
    }
    let mut stream = response.bytes_stream();
    let mut buffered = Vec::new();
    let mut assembler = Assembler {
        tool_blocks,
        ..Default::default()
    };
    let mut finished = false;
    // Reset the watchdog after every received chunk, rather than applying a
    // total deadline to a healthy multi-minute model generation.
    while let Some(part) = tokio::time::timeout(idle_timeout, stream.next())
        .await
        .context("Model stream idle timeout")?
    {
        buffered.extend_from_slice(&part?);
        // A single network read can contain many valid small SSE frames.
        // Enforce the bound only on the unconsumed frame after draining.
        while let Some((position, separator)) = boundary(&buffered) {
            let frame = buffered[..position].to_vec();
            buffered.drain(..position + separator);
            if consume_frame(&frame, &mut assembler, &deltas)? {
                finished = true;
                break;
            }
        }
        if finished {
            break;
        }
        if buffered.len() > 4_000_000 {
            bail!("Model SSE frame exceeds maximum size")
        }
    }
    if !buffered.is_empty() && !finished {
        consume_frame(&buffered, &mut assembler, &deltas)?;
    }
    // A dropped HTTP connection is not a completed assistant turn. In
    // particular, never dispatch a syntactically complete tool call from a
    // truncated SSE response unless the provider supplied a terminal marker.
    if !finished && assembler.finish_reason.is_none() {
        bail!("Model SSE connection closed before completion marker");
    }
    assembler.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_streamed_reasoning_for_deepseek_tool_followup() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut parser = Assembler::default();
        parser
            .consume(
                &json!({"choices":[{"delta":{
            "reasoning_content":"inspect "}}]}),
                &tx,
            )
            .unwrap();
        parser
            .consume(
                &json!({"choices":[{"delta":{
            "reasoning_content":"first",
            "tool_calls":[{"index":0,"id":"call_1","type":"function",
                "function":{"name":"read_file","arguments":"{}"}}]},
            "finish_reason":"tool_calls"}]}),
                &tx,
            )
            .unwrap();
        let response = parser.finish().unwrap();
        assert_eq!(
            response.message.reasoning_content.as_deref(),
            Some("inspect first")
        );
        assert_eq!(response.message.tool_calls.unwrap()[0].id, "call_1");
        assert!(
            rx.try_recv().is_err(),
            "reasoning must not leak to displayed text"
        );
    }

    #[test]
    fn joins_tool_call_fragments_by_index() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut parser = Assembler::default();
        parser
            .consume(
                &json!({"choices":[{"delta":{"tool_calls":[{
                    "index":0,"id":"abc","type":"function",
                    "function":{"name":"read_file","arguments":"{\"path\":"}
                }]}}]}),
                &tx,
            )
            .unwrap();
        parser
            .consume(
                &json!({"choices":[{"delta":{"tool_calls":[{
            "index":0,"function":{"arguments":"\"a.rs\"}"}
        }]},"finish_reason":"tool_calls"}]}),
                &tx,
            )
            .unwrap();
        parser
            .consume(
                &json!({"choices":[],"usage":{
                    "prompt_tokens":100,"completion_tokens":10,"total_tokens":110,
                    "prompt_tokens_details":{"cached_tokens":40},
                    "cache_creation_input_tokens":15,
                    "server_tool_use":{"web_search_requests":3}
                }}),
                &tx,
            )
            .unwrap();
        let response = parser.finish().unwrap();
        let usage = response.usage.unwrap();
        assert_eq!(usage.cache_read_input_tokens, 40);
        assert_eq!(usage.cache_creation_input_tokens, 15);
        assert_eq!(usage.web_search_requests, 3);
        let call = &response.message.tool_calls.unwrap()[0];
        assert_eq!(call.id, "abc");
        assert_eq!(call.function.name, "read_file");
        assert_eq!(call.function.arguments, "{\"path\":\"a.rs\"}");
    }
    #[test]
    fn completed_tools_remain_in_assistant_transcript_without_duplication() {
        let (deltas, _ignored) = tokio::sync::mpsc::unbounded_channel();
        let (blocks, mut incoming) = tokio::sync::mpsc::unbounded_channel();
        let mut assembler = Assembler {
            tool_blocks: Some(blocks),
            ..Default::default()
        };
        assembler
            .consume(
                &json!({"choices":[{"delta":{"tool_calls":[{
                    "index":0,"id":"tool_a","function":{"name":"read_file",
                        "arguments":"{\"file_path\":\"one\"}"}}
                ]}}]}),
                &deltas,
            )
            .unwrap();
        assembler
            .consume(
                &json!({"choices":[{"delta":{"tool_calls":[{
            "index":1,"id":"tool_b","function":{"name":"read_file",
                "arguments":"{\"file_path\":\"two\"}"}}
        ]},"finish_reason":"tool_calls"}]}),
                &deltas,
            )
            .unwrap();
        assert!(
            incoming.try_recv().is_err(),
            "indices do not mark a completed response"
        );
        let result = assembler.finish().unwrap();
        let calls = result.message.tool_calls.unwrap();
        assert_eq!(
            calls.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
            vec!["tool_a", "tool_b"]
        );
        assert_eq!(incoming.try_recv().unwrap().id, "tool_a");
        assert_eq!(incoming.try_recv().unwrap().id, "tool_b");
        assert!(
            incoming.try_recv().is_err(),
            "tool ids must not be emitted twice"
        );
    }

    #[tokio::test]
    async fn reads_real_split_sse_http_stream() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut connection, _) = listener.accept().await.unwrap();
            crate::test_http::consume_request(&mut connection)
                .await
                .unwrap();
            let payload=concat!(
                "data: {\"choices\":[{\"delta\":{\"content\":\"Hel\"}}]}\n\n",
                "data: {\"choices\":[{\"delta\":{\"content\":\"lo\"},\"finish_reason\":\"stop\"}]}\n\n",
                "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":2,\"total_tokens\":5}}\n\n",
                "data: [DONE]\n\n"
            );
            connection.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",payload.len()).as_bytes()).await.unwrap();
            for chunk in payload.as_bytes().chunks(17) {
                connection.write_all(chunk).await.unwrap();
            }
        });
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let reply = complete(
            &Client::new(),
            &format!("http://{address}/v1"),
            None,
            &ModelRequest {
                model: "mock".into(),
                system: "test".into(),
                messages: vec![Message::text("user", "hello")],
                tools: vec![],
            },
            tx,
            None,
            &crate::config::ModelTuning::default(),
        )
        .await
        .unwrap();
        server.await.unwrap();
        let mut deltas = String::new();
        while let Ok(delta) = rx.try_recv() {
            deltas.push_str(&delta);
        }
        assert_eq!(deltas, "Hello");
        assert_eq!(reply.message.content.as_deref(), Some("Hello"));
        assert_eq!(reply.usage.unwrap().total_tokens, 5);
    }
    #[tokio::test]
    async fn stream_retries_capacity_before_first_delta_without_duplication() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            for attempt in 0..2 {
                let (mut socket, _) = listener.accept().await.unwrap();
                crate::test_http::consume_request(&mut socket)
                    .await
                    .unwrap();
                let (status, body) = if attempt == 0 {
                    (
                        "503 Service Unavailable",
                        r#"{"error":"temporary"}"#.to_string(),
                    )
                } else {
                    ("200 OK",concat!(
                        "data: {\"choices\":[{\"delta\":{\"content\":\"Recovered\"},\"finish_reason\":\"stop\"}]}\n\n",
                        "data: [DONE]\n\n").to_string())
                };
                socket.write_all(format!(
                    "HTTP/1.1 {status}\r\nRetry-After: 0\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()).as_bytes()).await.unwrap();
            }
        });
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let response = complete(
            &Client::new(),
            &format!("http://{address}/v1"),
            None,
            &ModelRequest {
                model: "mock".into(),
                system: "system".into(),
                messages: vec![Message::text("user", "recover")],
                tools: vec![],
            },
            tx,
            None,
            &crate::config::ModelTuning::default(),
        )
        .await
        .unwrap();
        server.await.unwrap();
        assert_eq!(response.message.content.as_deref(), Some("Recovered"));
        assert_eq!(rx.try_recv().unwrap(), "Recovered");
        assert!(
            rx.try_recv().is_err(),
            "retry must not emit a duplicate delta"
        );
    }
}
