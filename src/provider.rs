use crate::{config::ProviderProfile, protocol::{Message, ModelRequest, ModelResponse, ToolCall, FunctionCall, TokenUsage}};
use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use std::{collections::HashMap, time::Duration};
use tokio::sync::mpsc::UnboundedSender;

#[async_trait]
pub trait ModelProvider: Send + Sync {
    async fn complete(&self, request: &ModelRequest) -> Result<ModelResponse>;
    async fn complete_stream(&self, request:&ModelRequest, _deltas:UnboundedSender<String>) -> Result<ModelResponse>{
        self.complete(request).await
    }
}

pub fn create(profile: &ProviderProfile) -> Result<Box<dyn ModelProvider>> {
    let key = match &profile.api_key_env {
        Some(env) => Some(std::env::var(env)
            .with_context(|| format!("Provider '{}' requires environment variable {env}", profile.id))?),
        None => None,
    };
    let client = Client::builder().timeout(Duration::from_secs(180)).build()?;
    match profile.kind.as_str() {
        "openai-compatible" => Ok(Box::new(OpenAiCompatible { client, base_url: profile.base_url.clone(), key })),
        "gemini" => Ok(Box::new(Gemini { client, base_url: profile.base_url.clone(), key: key.context("Gemini endpoint requires an API key")? })),
        other => bail!("Unsupported provider kind: {other}"),
    }
}

struct OpenAiCompatible {
    client: Client,
    base_url: String,
    key: Option<String>,
}

#[async_trait]
impl ModelProvider for OpenAiCompatible {
    async fn complete(&self, request: &ModelRequest) -> Result<ModelResponse> {
        let mut messages = vec![Message::text("system", &request.system)];
        messages.extend_from_slice(&request.messages);
        let mut body = json!({"model":request.model,"messages":messages});
        if !request.tools.is_empty() {
            body["tools"] = json!(request.tools);
            body["tool_choice"] = json!("auto");
        }
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let mut post = self.client.post(url).json(&body);
        if let Some(key) = &self.key {
            post = post.bearer_auth(key);
        }
        let response = post.send().await.context("Model request failed")?;
        let status = response.status();
        let value: Value = read_response(response).await?;
        if !status.is_success() { bail!("Model HTTP {status}: {}", error_summary(&value)); }
        let choice = value.get("choices").and_then(Value::as_array)
            .and_then(|a| a.first()).context("Model returned no choices")?;
        let mut message: Message = serde_json::from_value(choice.get("message")
            .context("Model returned no message")?.clone())
            .context("Invalid model message")?;
        if message.role != "assistant" { bail!("Unexpected response role: {}", message.role); }
        if matches!(message.tool_calls.as_ref(), Some(v) if v.is_empty()) {
            message.tool_calls = None;
        }
        Ok(ModelResponse {
            message,
            finish_reason: choice.get("finish_reason").and_then(Value::as_str).map(str::to_owned),
            usage: parse_openai_usage(&value),
        })
    }

    async fn complete_stream(&self, request:&ModelRequest, deltas:UnboundedSender<String>) -> Result<ModelResponse>{
        crate::sse::complete(&self.client,&self.base_url,self.key.as_deref(),request,deltas).await
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
                let mut p: Vec<Value> = msg.content.as_ref().filter(|s| !s.is_empty())
                    .map(|s| vec![json!({"text":s})]).unwrap_or_default();
                for call in msg.tool_calls.as_deref().unwrap_or(&[]) {
                    let args: Value = serde_json::from_str(&call.function.arguments)
                        .with_context(|| format!("Invalid arguments for {}", call.function.name))?;
                    p.push(json!({"functionCall":{"name": call.function.name, "args":args}}));
                }
                p
            }
            "tool" => {
                let id = msg.tool_call_id.as_ref().context("Tool result missing call id")?;
                let name = names.get(id).context("Tool response has no matching call")?;
                vec![json!({"functionResponse":{"name":name,"response":{"content":msg.content}}})]
            }
            _ => vec![json!({"text":msg.content.as_deref().unwrap_or("")})],
        };
        if parts.is_empty() { continue; }
        contents.push(json!({"role": if msg.role == "assistant" {"model"} else {"user"}, "parts":parts}));
    }
    let declarations: Vec<Value> = request.tools.iter().filter_map(|t| {
        let f = t.get("function")?;
        Some(json!({
            "name": f.get("name")?,
            "description": f.get("description")?,
            "parameters": f.get("parameters")?,
        }))
    }).collect();
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
}

#[async_trait]
impl ModelProvider for Gemini {
    async fn complete(&self, request: &ModelRequest) -> Result<ModelResponse> {
        let body = gemini_request_body(request)?;
        let model = request.model.strip_prefix("models/").unwrap_or(&request.model);
        let url = format!("{}/models/{}:generateContent", self.base_url.trim_end_matches('/'), model);
        let response = self.client.post(url)
            .header("x-goog-api-key", &self.key)
            .json(&body).send().await.context("Gemini request failed")?;
        let status = response.status();
        let value = read_response(response).await?;
        if !status.is_success() { bail!("Model HTTP {status}: {}", error_summary(&value)); }
        let candidate = value.get("candidates").and_then(Value::as_array)
            .and_then(|a| a.first()).context("Model returned no candidates")?;
        let parts = candidate.pointer("/content/parts").and_then(Value::as_array)
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
                        name: f.get("name").and_then(Value::as_str)
                            .context("Function call missing name")?.to_owned(),
                        arguments: serde_json::to_string(f.get("args").unwrap_or(&json!({})))?,
                    },
                });
            }
        }
        Ok(ModelResponse {
            message: Message {
                role: "assistant".into(),
                content: if texts.is_empty() { None } else { Some(texts.join("\n")) },
                tool_calls: if calls.is_empty() { None } else { Some(calls) },
                tool_call_id: None,
            },
            finish_reason: candidate.get("finishReason").and_then(Value::as_str).map(str::to_owned),
            usage: parse_gemini_usage(&value),
        })
    }

    async fn complete_stream(&self, request:&ModelRequest, deltas:UnboundedSender<String>) -> Result<ModelResponse> {
        let body=gemini_request_body(request)?;
        crate::gemini_sse::complete(&self.client,&self.base_url,&self.key,
            &request.model,&body,deltas).await
    }
}

fn parse_openai_usage(value: &Value) -> Option<TokenUsage> {
    let usage = value.get("usage")?;
    let input = usage.get("prompt_tokens").and_then(Value::as_u64).unwrap_or(0);
    let output = usage.get("completion_tokens").and_then(Value::as_u64).unwrap_or(0);
    Some(TokenUsage {
        input_tokens: input,
        output_tokens: output,
        total_tokens: usage.get("total_tokens").and_then(Value::as_u64)
            .unwrap_or(input.saturating_add(output)),
    })
}

fn parse_gemini_usage(value: &Value) -> Option<TokenUsage> {
    let usage = value.get("usageMetadata")?;
    let input = usage.get("promptTokenCount").and_then(Value::as_u64).unwrap_or(0);
    let output = usage.get("candidatesTokenCount").and_then(Value::as_u64).unwrap_or(0);
    Some(TokenUsage {
        input_tokens: input,
        output_tokens: output,
        total_tokens: usage.get("totalTokenCount").and_then(Value::as_u64)
            .unwrap_or(input.saturating_add(output)),
    })
}

async fn read_response(response: reqwest::Response) -> Result<Value> {
    let body = response.text().await?;
    serde_json::from_str(&body).map_err(|_| anyhow!("Provider returned non-JSON: {}", body.chars().take(500).collect::<String>()))
}

fn error_summary(value: &Value) -> String {
    value.pointer("/error/message").and_then(Value::as_str)
        .or_else(|| value.get("message").and_then(Value::as_str))
        .map(str::to_owned).unwrap_or_else(|| value.to_string().chars().take(500).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gemini_stream_and_nonstream_use_identical_request_body() {
        let request=ModelRequest{
            model:"test-model".into(),
            system:"System instructions".into(),
            messages:vec![
                Message::text("user","Inspect source"),
                Message::text("assistant","Will read"),
            ],
            tools:vec![json!({"type":"function","function":{
                "name":"read_file","description":"Read text",
                "parameters":{"type":"object","properties":{"path":{"type":"string"}}}
            }})],
        };
        let body=gemini_request_body(&request).unwrap();
        assert_eq!(body["systemInstruction"]["parts"][0]["text"],"System instructions");
        assert_eq!(body["contents"][0]["role"],"user");
        assert_eq!(body["tools"][0]["functionDeclarations"][0]["name"],"read_file");
    }
    #[test]
    fn token_usage_normalization_is_provider_neutral() {
        assert_eq!(parse_openai_usage(&json!({"usage":{"prompt_tokens":20,
            "completion_tokens":3,"total_tokens":23}})),
            Some(TokenUsage {input_tokens:20,output_tokens:3,total_tokens:23}));
        assert_eq!(parse_gemini_usage(&json!({"usageMetadata":{"promptTokenCount":7,
            "candidatesTokenCount":9,"totalTokenCount":16}})),
            Some(TokenUsage {input_tokens:7,output_tokens:9,total_tokens:16}));
        assert!(parse_gemini_usage(&json!({})).is_none());
    }

    #[test]
    fn parse_provider_response_with_tool_call() {
        let msg: Message = serde_json::from_value(json!({
            "role":"assistant","content":null,
            "tool_calls":[{"id":"call1","type":"function",
            "function":{"name":"read_file","arguments":"{\"path\":\"a.txt\"}"}}]
        })).unwrap();
        assert_eq!(msg.tool_calls.unwrap()[0].function.name, "read_file");
    }
}
