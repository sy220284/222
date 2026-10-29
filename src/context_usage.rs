//! Context window inspection from 2.1.88 commands/context/context-noninteractive.ts
//! and utils/analyzeContext.ts. Counts the actual model request snapshot;
//! provider tokenizers may differ from the UTF-16 rough estimation fallback.
use crate::protocol::{Message, ModelRequest, TokenUsage};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Clone)]
pub struct Snapshot {
    pub model: String,
    pub system_tokens: u64,
    pub built_in_tokens: u64,
    pub mcp_tokens: u64,
    pub user_tokens: u64,
    pub assistant_tokens: u64,
    pub tool_call_tokens: u64,
    pub tool_result_tokens: u64,
    pub attachment_tokens: u64,
    pub tool_calls: BTreeMap<String, (u64, u64)>,
    pub mcp_tools: Vec<Value>,
}

fn estimate(text: &str) -> u64 {
    // Source services/tokenEstimation.ts::roughTokenCountEstimation:
    // JS string length is measured in UTF-16 code units, rounded / 4.
    (text.encode_utf16().count() as u64 + 2) / 4
}

fn block_tokens(block: &Value) -> (u64, u64) {
    if let Some(text) = block.as_str() {
        return (estimate(text), 0);
    }
    match block.get("type").and_then(Value::as_str) {
        Some("text" | "input_text" | "output_text") => (
            block.get("text").and_then(Value::as_str).map(estimate).unwrap_or(0),
            0,
        ),
        Some("image" | "document") => (0, 2_000),
        Some("tool_result") => match block.get("content") {
            Some(Value::String(s)) => (estimate(s), 0),
            Some(Value::Array(parts)) => parts.iter().fold((0, 0), |(t, a), p| {
                let (next_t, next_a) = block_tokens(p);
                (t + next_t, a + next_a)
            }),
            _ => (0, 0),
        },
        Some("tool_use") => (
            block.get("name").and_then(Value::as_str).map(estimate).unwrap_or(0)
                + block.get("input").map(|v| estimate(&v.to_string())).unwrap_or(0),
            0,
        ),
        _ => (0, 0),
    }
}

fn message_content_tokens(message: &Message) -> (u64, u64) {
    if let Some(blocks) = message.content_blocks.as_ref() {
        return blocks.iter().fold((0, 0), |(text, attachment), block| {
            let (t, a) = block_tokens(block);
            (text + t, attachment + a)
        });
    }
    (message.content.as_deref().map(estimate).unwrap_or(0), 0)
}

impl Snapshot {
    pub fn from_request(request: &ModelRequest) -> Self {
        let mut result = Self {
            model: request.model.clone(),
            system_tokens: estimate(&request.system),
            built_in_tokens: 0,
            mcp_tokens: 0,
            user_tokens: 0,
            assistant_tokens: 0,
            tool_call_tokens: 0,
            tool_result_tokens: 0,
            attachment_tokens: 0,
            tool_calls: BTreeMap::new(),
            mcp_tools: Vec::new(),
        };
        for definition in &request.tools {
            let name = definition.pointer("/function/name")
                .or_else(|| definition.get("name"))
                .and_then(Value::as_str).unwrap_or("");
            let tokens = estimate(&definition.to_string());
            if let Some(server) = name.strip_prefix("mcp__").and_then(|s| s.split("__").next()) {
                result.mcp_tokens += tokens;
                result.mcp_tools.push(json!({"name":name,"serverName":server,
                    "tokens":tokens,"isLoaded":true}));
            } else {
                result.built_in_tokens += tokens;
            }
        }
        for message in &request.messages {
            let (content, attachments) = message_content_tokens(message);
            result.attachment_tokens += attachments;
            match message.role.as_str() {
                "assistant" => result.assistant_tokens += content,
                "tool" => result.tool_result_tokens += content,
                _ => result.user_tokens += content,
            }
            if let Some(calls) = message.tool_calls.as_ref() {
                for call in calls {
                    let tokens = estimate(&call.function.name)
                        + estimate(&call.function.arguments);
                    result.tool_call_tokens += tokens;
                    result.tool_calls.entry(call.function.name.clone()).or_default().0 += tokens;
                }
            }
            if message.role == "tool" {
                let name = message.tool_call_id.as_deref().unwrap_or("tool");
                result.tool_calls.entry(name.to_owned()).or_default().1 += content;
            }
        }
        result
    }

    pub fn total_tokens(&self) -> u64 {
        self.system_tokens + self.built_in_tokens + self.mcp_tokens
            + self.user_tokens + self.assistant_tokens + self.tool_call_tokens
            + self.tool_result_tokens + self.attachment_tokens
    }

    pub fn to_json(&self, usage: TokenUsage, from_last_request: bool) -> Value {
        let window = std::env::var("ATLAS_CONTEXT_WINDOW_TOKENS").ok()
            .and_then(|raw| raw.parse::<u64>().ok()).filter(|v| *v > 0);
        let total = self.total_tokens();
        let mut categories = vec![
            json!({"name":"System prompt","tokens":self.system_tokens,"color":"secondaryText"}),
            json!({"name":"System tools","tokens":self.built_in_tokens,"color":"cyan"}),
            json!({"name":"MCP tools","tokens":self.mcp_tokens,"color":"purple"}),
            json!({"name":"User messages","tokens":self.user_tokens,"color":"green"}),
            json!({"name":"Assistant messages","tokens":self.assistant_tokens,"color":"blue"}),
            json!({"name":"Tool calls","tokens":self.tool_call_tokens,"color":"yellow"}),
            json!({"name":"Tool results","tokens":self.tool_result_tokens,"color":"orange"}),
            json!({"name":"Attachments","tokens":self.attachment_tokens,"color":"magenta"}),
        ];
        if let Some(limit) = window {
            categories.push(json!({"name":"Free space","tokens":limit.saturating_sub(total),
                "color":"secondaryText"}));
        }
        let breakdown = self.tool_calls.iter().map(|(name, (call, result))|
            json!({"name":name,"callTokens":call,"resultTokens":result})
        ).collect::<Vec<_>>();
        let max = window.unwrap_or(0);
        json!({
            "categories":categories,
            "totalTokens":total,
            "maxTokens":max,
            "rawMaxTokens":max,
            "percentage":if max > 0 { (total as f64 * 100.0 / max as f64).round() } else { 0.0 },
            "gridRows":[],
            "model":self.model,
            "memoryFiles":[],
            "mcpTools":self.mcp_tools,
            "agents":[],
            "isAutoCompactEnabled":false,
            "messageBreakdown":{
                "toolCallTokens":self.tool_call_tokens,
                "toolResultTokens":self.tool_result_tokens,
                "attachmentTokens":self.attachment_tokens,
                "assistantMessageTokens":self.assistant_tokens,
                "userMessageTokens":self.user_tokens,
                "toolCallsByType":breakdown,
                "attachmentsByType":[]
            },
            "apiUsage":{
                "input_tokens":usage.input_tokens,
                "output_tokens":usage.output_tokens,
                "cache_creation_input_tokens":usage.cache_creation_input_tokens,
                "cache_read_input_tokens":usage.cache_read_input_tokens
            },
            "estimateOnly":true,
            "windowKnown":window.is_some(),
            "fromLastModelRequest":from_last_request
        })
    }

    pub fn render(&self, usage: TokenUsage, from_last_request: bool) -> String {
        let data = self.to_json(usage, from_last_request);
        let mut output = format!("## Context Usage\n\nModel: {}\nEstimated tokens: {}\n",
            self.model, self.total_tokens());
        if data["windowKnown"] == true {
            output.push_str(&format!("Context window: {} ({}%)\n",
                data["rawMaxTokens"], data["percentage"]));
        } else {
            output.push_str("Context window: unknown (set ATLAS_CONTEXT_WINDOW_TOKENS for percentage)\n");
        }
        if !from_last_request {
            output.push_str("Snapshot: reconstructed from current session (no model request yet)\n");
        }
        output.push_str("Token counts are estimates; provider tokenization may differ.\n\n");
        for category in data["categories"].as_array().into_iter().flatten() {
            if category["tokens"].as_u64().unwrap_or(0) > 0 {
                output.push_str(&format!("{}: {}\n", category["name"].as_str().unwrap_or(""),
                    category["tokens"]));
            }
        }
        output
    }
}
