use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Message {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Provider-bound reasoning state: DeepSeek reasoning text, or versioned
    /// opaque Responses/Gemini envelopes. Kept in sessions, never displayed
    /// as answer text or forwarded to a different provider's API.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    /// SDK multimodal content, stored durably in sessions and translated
    /// into each model provider's native content blocks at the wire boundary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_blocks: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl Message {
    /// Extract the visible text of a structured provider message. This
    /// mirrors the original getAssistantMessageText block ordering while
    /// retaining flattened text for OpenAI-compatible transports.
    pub fn visible_text(&self) -> String {
        if let Some(blocks) = self.content_blocks.as_ref() {
            let parts = blocks
                .iter()
                .filter_map(|block| {
                    if let Some(text) = block.as_str() {
                        return Some(text);
                    }
                    if block.get("type").and_then(serde_json::Value::as_str) == Some("text") {
                        block.get("text").and_then(serde_json::Value::as_str)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            if !parts.is_empty() {
                return parts.join("\n");
            }
        }
        self.content.clone().unwrap_or_default()
    }

    pub fn text(role: &str, content: impl Into<String>) -> Self {
        Self {
            role: role.to_owned(),
            content: Some(content.into()),
            reasoning_content: None,
            content_blocks: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn tool_result(call: &ToolCall, content: String) -> Self {
        Self {
            role: "tool".into(),
            content: Some(content),
            reasoning_content: None,
            content_blocks: None,
            tool_calls: None,
            tool_call_id: Some(call.id.clone()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ModelRequest {
    pub model: String,
    pub system: String,
    pub messages: Vec<Message>,
    pub tools: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub cache_read_input_tokens: u64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub cache_creation_input_tokens: u64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub web_search_requests: u64,
}
fn is_zero(v: &u64) -> bool {
    *v == 0
}

impl TokenUsage {
    pub fn accumulate(&mut self, other: Self) {
        self.input_tokens = self.input_tokens.saturating_add(other.input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(other.output_tokens);
        self.total_tokens = self.total_tokens.saturating_add(other.total_tokens);
        self.cache_read_input_tokens = self
            .cache_read_input_tokens
            .saturating_add(other.cache_read_input_tokens);
        self.cache_creation_input_tokens = self
            .cache_creation_input_tokens
            .saturating_add(other.cache_creation_input_tokens);
        self.web_search_requests = self
            .web_search_requests
            .saturating_add(other.web_search_requests);
    }
}

#[derive(Debug, Clone)]
pub struct ModelResponse {
    pub message: Message,
    pub finish_reason: Option<String>,
    pub usage: Option<TokenUsage>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn deepseek_reasoning_round_trips_without_becoming_visible_text() {
        let message: Message = serde_json::from_value(json!({
            "role": "assistant",
            "content": null,
            "reasoning_content": "First inspect the file",
            "tool_calls": [{"id":"call_1","type":"function",
                "function":{"name":"read_file","arguments":"{}"}}]
        }))
        .unwrap();
        assert_eq!(
            message.reasoning_content.as_deref(),
            Some("First inspect the file")
        );
        assert_eq!(message.visible_text(), "");
        let restored: Message =
            serde_json::from_value(serde_json::to_value(&message).unwrap()).unwrap();
        assert_eq!(restored, message);
        assert_eq!(
            serde_json::to_value(Message::text("user", "hello"))
                .unwrap()
                .get("reasoning_content"),
            None
        );
    }
}
