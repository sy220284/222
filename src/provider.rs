use crate::{
    config::ProviderProfile,
    protocol::{FunctionCall, Message, ModelRequest, ModelResponse, TokenUsage, ToolCall},
};
use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use std::{collections::HashMap, time::Duration};
use tokio::sync::mpsc::UnboundedSender;

#[async_trait]
pub trait ModelProvider: Send + Sync {
    /// Token-precise SDK thinking budgets are optional provider capabilities.
    /// Reject unsupported requests rather than claiming the setting took effect.
    fn set_thinking_budget(&mut self, budget: Option<u32>) -> Result<()> {
        if budget.is_some() {
            bail!("Selected model provider has no token-precise thinking budget capability");
        }
        Ok(())
    }
    async fn complete(&self, request: &ModelRequest) -> Result<ModelResponse>;
    async fn complete_stream(
        &self,
        request: &ModelRequest,
        _deltas: UnboundedSender<String>,
    ) -> Result<ModelResponse> {
        self.complete(request).await
    }
}

pub fn create(profile: &ProviderProfile) -> Result<Box<dyn ModelProvider>> {
    let key = match &profile.api_key_env {
        Some(env) => Some(std::env::var(env).with_context(|| {
            format!(
                "Provider '{}' requires environment variable {env}",
                profile.id
            )
        })?),
        None => None,
    };
    // A client-wide timeout measures the entire streamed completion. It
    // aborts healthy long-running generations after 180 seconds even while
    // tokens are arriving. Streaming adapters enforce per-chunk idle timeouts;
    // non-streaming requests retain their own bounded deadline.
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .build()?;
    match profile.kind.as_str() {
        "openai-compatible" => Ok(Box::new(OpenAiCompatible {
            client,
            base_url: profile.base_url.clone(),
            key,
        })),
        "gemini" => Ok(Box::new(Gemini {
            client,
            base_url: profile.base_url.clone(),
            key: key.context("Gemini endpoint requires an API key")?,
            thinking_budget: None,
        })),
        other => bail!("Unsupported provider kind: {other}"),
    }
}

/// Retry only transient failures before a model response starts. The request
/// body is cloned unchanged: no tool is executed or session turn appended
/// during a retry. HTTP failures with an explicit no-retry header are final.
/// Once an SSE response has started, its parser owns the stream and we never
/// replay a partially delivered completion.
pub(crate) fn stream_idle_timeout() -> Duration {
    let millis = std::env::var("ATLAS_STREAM_IDLE_TIMEOUT_MS")
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
        .filter(|millis| *millis > 0)
        .unwrap_or(90_000);
    Duration::from_millis(millis)
}

pub(crate) async fn send_model_request(
    request: reqwest::RequestBuilder,
) -> Result<reqwest::Response> {
    const ATTEMPTS: usize = 4;
    for attempt in 0..ATTEMPTS {
        let attempt_request = request
            .try_clone()
            .context("Model request body cannot be replayed safely")?;
        match attempt_request.send().await {
            Ok(response) => {
                let status = response.status();
                let header = response
                    .headers()
                    .get("x-should-retry")
                    .and_then(|v| v.to_str().ok());
                let retryable = header != Some("false")
                    && (header == Some("true")
                        || matches!(
                            status.as_u16(),
                            408 | 409 | 429 | 500 | 502 | 503 | 504 | 529
                        ));
                if retryable && attempt + 1 < ATTEMPTS {
                    let delay = response
                        .headers()
                        .get(reqwest::header::RETRY_AFTER)
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.trim().parse::<u64>().ok())
                        // withRetry.ts::getRetryDelay honors the server's
                        // Retry-After directive even when it exceeds normal
                        // exponential backoff. Retrying after only 30 seconds
                        // on a longer rate-limit window repeatedly fails.
                        .map(|seconds| Duration::from_secs(seconds.min(6 * 60 * 60)))
                        .unwrap_or_else(|| Duration::from_millis((500u64 << attempt).min(32_000)));
                    // Drop the failed HTTP response before waiting; do not
                    // retain a busy connection across capacity retries.
                    drop(response);
                    tokio::time::sleep(delay).await;
                    continue;
                }
                return Ok(response);
            }
            Err(error) if attempt + 1 < ATTEMPTS && (error.is_connect() || error.is_timeout()) => {
                tokio::time::sleep(Duration::from_millis(200u64 << attempt)).await;
            }
            Err(error) => return Err(error.into()),
        }
    }
    unreachable!("retry loop returns on its final attempt")
}

/// Serialize session messages to provider-native blocks without leaking the
/// internal persisted content_blocks attribute to an OpenAI-compatible API.
pub(crate) fn openai_messages(request: &ModelRequest) -> Result<Vec<Value>> {
    let mut messages = Vec::with_capacity(request.messages.len() + 1);
    for msg in std::iter::once(Message::text("system", &request.system))
        .chain(request.messages.iter().cloned())
    {
        let mut wire = serde_json::to_value(&msg)?;
        if let Some(blocks) = msg.content_blocks.as_deref() {
            wire["content"] = json!(crate::multimodal::openai(blocks)?);
            if let Some(map) = wire.as_object_mut() {
                map.remove("content_blocks");
            }
        }
        messages.push(wire);
    }
    Ok(messages)
}

struct OpenAiCompatible {
    client: Client,
    base_url: String,
    key: Option<String>,
}

#[async_trait]
impl ModelProvider for OpenAiCompatible {
    async fn complete(&self, request: &ModelRequest) -> Result<ModelResponse> {
        let mut body = json!({"model":request.model,
            "messages":openai_messages(request)?});
        if !request.tools.is_empty() {
            body["tools"] = json!(request.tools);
            body["tool_choice"] = json!("auto");
        }
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let mut post = self.client.post(url)
            .timeout(Duration::from_secs(180))
            .json(&body);
        if let Some(key) = &self.key {
            post = post.bearer_auth(key);
        }
        let response = send_model_request(post)
            .await
            .context("Model request failed")?;
        let value: Value = read_model_response(response).await?;
        let choice = value
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .context("Model returned no choices")?;
        if let Some(refusal) = choice.pointer("/message/refusal").and_then(Value::as_str) {
            bail!("Model refused the request: {}", refusal.chars().take(500).collect::<String>());
        }
        let mut message: Message = serde_json::from_value(
            choice
                .get("message")
                .context("Model returned no message")?
                .clone(),
        )
        .context("Invalid model message")?;
        if message.role != "assistant" {
            bail!("Unexpected response role: {}", message.role);
        }
        if matches!(message.tool_calls.as_ref(), Some(v) if v.is_empty()) {
            message.tool_calls = None;
        }
        Ok(ModelResponse {
            message,
            finish_reason: choice
                .get("finish_reason")
                .and_then(Value::as_str)
                .map(str::to_owned),
            usage: parse_openai_usage(&value),
        })
    }

    async fn complete_stream(
        &self,
        request: &ModelRequest,
        deltas: UnboundedSender<String>,
    ) -> Result<ModelResponse> {
        crate::sse::complete(
            &self.client,
            &self.base_url,
            self.key.as_deref(),
            request,
            deltas,
        )
        .await
    }
}

fn gemini_request_body(request: &ModelRequest) -> Result<Value> {
    let mut names = HashMap::<String, String>::new();
    for msg in &request.messages {
        for call in msg.tool_calls.as_deref().unwrap_or(&[]) {
            names.insert(call.id.clone(), call.function.name.clone());
        }
    }
    let mut contents = Vec::new();
    for msg in &request.messages {
        let parts: Vec<Value> = match msg.role.as_str() {
            "assistant" => {
                let mut p: Vec<Value> = msg
                    .content
                    .as_ref()
                    .filter(|s| !s.is_empty())
                    .map(|s| vec![json!({"text":s})])
                    .unwrap_or_default();
                for call in msg.tool_calls.as_deref().unwrap_or(&[]) {
                    let args: Value = serde_json::from_str(&call.function.arguments)
                        .with_context(|| format!("Invalid arguments for {}", call.function.name))?;
                    p.push(json!({"functionCall":{"name": call.function.name, "args":args}}));
                }
                p
            }
            "tool" => {
                let id = msg
                    .tool_call_id
                    .as_ref()
                    .context("Tool result missing call id")?;
                let name = names
                    .get(id)
                    .context("Tool response has no matching call")?;
                vec![json!({"functionResponse":{"name":name,"response":{"content":msg.content}}})]
            }
            _ => {
                if let Some(blocks) = msg.content_blocks.as_deref() {
                    crate::multimodal::gemini(blocks)?
                } else {
                    vec![json!({"text":msg.content.as_deref().unwrap_or("")})]
                }
            }
        };
        if parts.is_empty() {
            continue;
        }
        contents.push(
            json!({"role": if msg.role == "assistant" {"model"} else {"user"}, "parts":parts}),
        );
    }
    let declarations: Vec<Value> = request
        .tools
        .iter()
        .filter_map(|t| {
            let f = t.get("function")?;
            Some(json!({
                "name": f.get("name")?,
                "description": f.get("description")?,
                "parameters": f.get("parameters")?,
            }))
        })
        .collect();
    let mut body = json!({
        "systemInstruction":{"parts":[{"text":request.system}]},
        "contents":contents
    });
    if !declarations.is_empty() {
        body["tools"] = json!([{"functionDeclarations":declarations}]);
    }
    Ok(body)
}

struct Gemini {
    client: Client,
    base_url: String,
    key: String,
    thinking_budget: Option<u32>,
}

#[async_trait]
impl ModelProvider for Gemini {
    fn set_thinking_budget(&mut self, budget: Option<u32>) -> Result<()> {
        self.thinking_budget = budget;
        Ok(())
    }

    async fn complete(&self, request: &ModelRequest) -> Result<ModelResponse> {
        let mut body = gemini_request_body(request)?;
        if let Some(budget) = self.thinking_budget {
            body["generationConfig"] = json!({"thinkingConfig":{"thinkingBudget":budget}});
        }
        let model = request
            .model
            .strip_prefix("models/")
            .unwrap_or(&request.model);
        let url = format!(
            "{}/models/{}:generateContent",
            self.base_url.trim_end_matches('/'),
            model
        );
        let response = self
            .client
            .post(url)
            .timeout(Duration::from_secs(180))
            .header("x-goog-api-key", &self.key)
            .json(&body);
        let response = send_model_request(response)
            .await
            .context("Gemini request failed")?;
        let value: Value = read_model_response(response).await?;
        // Gemini may reject the prompt before producing any candidates.
        // Preserve the explicit block reason instead of a misleading
        // "no candidates" error, and never enter the Agent tool loop.
        if let Some(reason) = value.pointer("/promptFeedback/blockReason").and_then(Value::as_str) {
            bail!("Model blocked the request: {reason}");
        }
        let candidate = value
            .get("candidates")
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .context("Model returned no candidates")?;
        let parts = candidate
            .pointer("/content/parts")
            .and_then(Value::as_array)
            .context("Model returned no content parts")?;
        let mut texts = Vec::new();
        let mut calls = Vec::new();
        for part in parts {
            if let Some(text) = part.get("text").and_then(Value::as_str) {
                texts.push(text.to_owned());
            }
            if let Some(f) = part.get("functionCall") {
                calls.push(ToolCall {
                    id: uuid::Uuid::new_v4().to_string(),
                    kind: "function".into(),
                    function: FunctionCall {
                        name: f
                            .get("name")
                            .and_then(Value::as_str)
                            .context("Function call missing name")?
                            .to_owned(),
                        arguments: serde_json::to_string(f.get("args").unwrap_or(&json!({})))?,
                    },
                });
            }
        }
        Ok(ModelResponse {
            message: Message {
                role: "assistant".into(),
                content: if texts.is_empty() {
                    None
                } else {
                    Some(texts.join("\n"))
                },
                content_blocks: None,
                tool_calls: if calls.is_empty() { None } else { Some(calls) },
                tool_call_id: None,
            },
            finish_reason: candidate
                .get("finishReason")
                .and_then(Value::as_str)
                .map(str::to_owned),
            usage: parse_gemini_usage(&value),
        })
    }

    async fn complete_stream(
        &self,
        request: &ModelRequest,
        deltas: UnboundedSender<String>,
    ) -> Result<ModelResponse> {
        let mut body = gemini_request_body(request)?;
        if let Some(budget) = self.thinking_budget {
            body["generationConfig"] = json!({"thinkingConfig":{"thinkingBudget":budget}});
        }
        crate::gemini_sse::complete(
            &self.client,
            &self.base_url,
            &self.key,
            &request.model,
            &body,
            deltas,
        )
        .await
    }
}

fn parse_openai_usage(value: &Value) -> Option<TokenUsage> {
    let usage = value.get("usage")?;
    let input = usage
        .get("prompt_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output = usage
        .get("completion_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    Some(TokenUsage {
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
    })
}

pub(crate) fn parse_gemini_usage(value: &Value) -> Option<TokenUsage> {
    let usage = value.get("usageMetadata")?;
    let input = usage
        .get("promptTokenCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output = usage
        .get("candidatesTokenCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    Some(TokenUsage {
        input_tokens: input,
        output_tokens: output,
        total_tokens: usage
            .get("totalTokenCount")
            .and_then(Value::as_u64)
            .unwrap_or(input.saturating_add(output)),
        cache_read_input_tokens: usage
            .get("cachedContentTokenCount")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        cache_creation_input_tokens: usage
            .get("cacheCreationInputTokenCount")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        web_search_requests: usage
            .get("webSearchRequests")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        ..Default::default()
    })
}

async fn read_model_response(response: reqwest::Response) -> Result<Value> {
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await?;
        let summary = serde_json::from_str::<Value>(&body)
            .map(|value| error_summary(&value))
            .unwrap_or_else(|_| body.chars().take(500).collect());
        bail!("Model HTTP {status}: {summary}");
    }
    read_response(response).await
}

async fn read_response(response: reqwest::Response) -> Result<Value> {
    let body = response.text().await?;
    serde_json::from_str(&body).map_err(|_| {
        anyhow!(
            "Provider returned non-JSON: {}",
            body.chars().take(500).collect::<String>()
        )
    })
}

fn error_summary(value: &Value) -> String {
    value
        .pointer("/error/message")
        .and_then(Value::as_str)
        .or_else(|| value.get("message").and_then(Value::as_str))
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string().chars().take(500).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn provider_adapters_preserve_user_image_blocks_and_keep_roles() {
        let parsed = crate::multimodal::parse(&json!([
            {"type":"text","text":"What is in this photo?"},
            {"type":"image","source":{"type":"base64","media_type":"image/png","data":"AQID"}}
        ]))
        .unwrap();
        let mut message = Message::text("user", parsed.preview);
        message.content_blocks = parsed.blocks;
        let req = ModelRequest {
            model: "vision".into(),
            system: "Be precise".into(),
            messages: vec![message],
            tools: vec![],
        };
        let openai = openai_messages(&req).unwrap();
        assert_eq!(openai[0]["role"], "system");
        assert_eq!(
            openai[1]["content"][1]["image_url"]["url"],
            "data:image/png;base64,AQID"
        );
        assert!(openai[1].get("content_blocks").is_none());
        let gemini = gemini_request_body(&req).unwrap();
        assert_eq!(
            gemini["contents"][0]["parts"][1]["inlineData"]["mimeType"],
            "image/png"
        );
        assert_eq!(
            gemini["contents"][0]["parts"][1]["inlineData"]["data"],
            "AQID"
        );
    }
    #[test]
    fn gemini_stream_and_nonstream_use_identical_request_body() {
        let request = ModelRequest {
            model: "test-model".into(),
            system: "System instructions".into(),
            messages: vec![
                Message::text("user", "Inspect source"),
                Message::text("assistant", "Will read"),
            ],
            tools: vec![json!({"type":"function","function":{
                "name":"read_file","description":"Read text",
                "parameters":{"type":"object","properties":{"path":{"type":"string"}}}
            }})],
        };
        let body = gemini_request_body(&request).unwrap();
        assert_eq!(
            body["systemInstruction"]["parts"][0]["text"],
            "System instructions"
        );
        assert_eq!(body["contents"][0]["role"], "user");
        assert_eq!(
            body["tools"][0]["functionDeclarations"][0]["name"],
            "read_file"
        );
    }
    #[test]
    fn token_usage_normalization_is_provider_neutral() {
        assert_eq!(
            parse_openai_usage(&json!({"usage":{"prompt_tokens":20,
            "completion_tokens":3,"total_tokens":23}})),
            Some(TokenUsage {
                input_tokens: 20,
                output_tokens: 3,
                total_tokens: 23,
                ..Default::default()
            })
        );
        assert_eq!(
            parse_gemini_usage(&json!({"usageMetadata":{"promptTokenCount":7,
            "candidatesTokenCount":9,"totalTokenCount":16}})),
            Some(TokenUsage {
                input_tokens: 7,
                output_tokens: 9,
                total_tokens: 16,
                ..Default::default()
            })
        );
        assert!(parse_gemini_usage(&json!({})).is_none());
        let openai = parse_openai_usage(&json!({"usage":{
            "prompt_tokens":100,"completion_tokens":15,"total_tokens":115,
            "prompt_tokens_details":{"cached_tokens":30},
            "cache_creation_input_tokens":10,
            "server_tool_use":{"web_search_requests":2}
        }}))
        .unwrap();
        assert_eq!(openai.cache_read_input_tokens, 30);
        assert_eq!(openai.cache_creation_input_tokens, 10);
        assert_eq!(openai.web_search_requests, 2);
        let gemini = parse_gemini_usage(&json!({"usageMetadata":{
            "promptTokenCount":200,"candidatesTokenCount":20,
            "cachedContentTokenCount":40,"cacheCreationInputTokenCount":12,
            "webSearchRequests":3
        }}))
        .unwrap();
        assert_eq!(gemini.cache_read_input_tokens, 40);
        assert_eq!(gemini.cache_creation_input_tokens, 12);
        assert_eq!(gemini.web_search_requests, 3);
    }

    #[test]
    fn parse_provider_response_with_tool_call() {
        let msg: Message = serde_json::from_value(json!({
            "role":"assistant","content":null,
            "tool_calls":[{"id":"call1","type":"function",
            "function":{"name":"read_file","arguments":"{\"path\":\"a.txt\"}"}}]
        }))
        .unwrap();
        assert_eq!(msg.tool_calls.unwrap()[0].function.name, "read_file");
    }
}
