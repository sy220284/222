//! Context window inspection from 2.1.88 commands/context/context-noninteractive.ts
//! and utils/analyzeContext.ts. Counts the actual model request snapshot;
//! provider tokenizers may differ from the UTF-16 rough estimation fallback.
use crate::protocol::{Message, ModelRequest, TokenUsage};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};

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
    pub system_tools: Vec<Value>,
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
            block
                .get("text")
                .and_then(Value::as_str)
                .map(estimate)
                .unwrap_or(0),
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
            block
                .get("name")
                .and_then(Value::as_str)
                .map(estimate)
                .unwrap_or(0)
                + block
                    .get("input")
                    .map(|v| estimate(&v.to_string()))
                    .unwrap_or(0),
            0,
        ),
        Some("thinking") => (
            block.get("thinking").and_then(Value::as_str).map(estimate).unwrap_or(0),
            0,
        ),
        Some("redacted_thinking") => (
            block.get("data").and_then(Value::as_str).map(estimate).unwrap_or(0),
            0,
        ),
        // tokenEstimation.ts::roughTokenCountEstimationForBlock counts
        // server_tool_use, web_search_tool_result and other text-like blocks
        // using their serialized form. Treating them as zero underreports the
        // active context and delays compaction for tool-heavy conversations.
        _ => (estimate(&block.to_string()), 0),
    }
}

fn message_content_tokens(message: &Message) -> (u64, u64) {
    if let Some(blocks) = message
        .content_blocks
        .as_ref()
        .filter(|blocks| !blocks.is_empty())
    {
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
            system_tools: Vec::new(),
        };
        for definition in &request.tools {
            let name = definition
                .pointer("/function/name")
                .or_else(|| definition.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("");
            let tokens = estimate(&definition.to_string());
            if let Some(server) = name
                .strip_prefix("mcp__")
                .and_then(|s| s.split("__").next())
            {
                result.mcp_tokens += tokens;
                result
                    .mcp_tools
                    .push(json!({"name":name,"serverName":server,
                    "tokens":tokens,"isLoaded":true}));
            } else {
                result.built_in_tokens += tokens;
                result
                    .system_tools
                    .push(json!({"name":name,"tokens":tokens}));
            }
        }
        result.count_messages(&request.messages);
        result
    }

    pub fn with_messages(&self, messages: &[Message]) -> Self {
        let mut current = self.clone();
        current.user_tokens = 0;
        current.assistant_tokens = 0;
        current.tool_call_tokens = 0;
        current.tool_result_tokens = 0;
        current.attachment_tokens = 0;
        current.tool_calls.clear();
        current.count_messages(messages);
        current
    }

    fn count_messages(&mut self, messages: &[Message]) {
        let mut tool_names: HashMap<&str, &str> = HashMap::new();
        for message in messages {
            if let Some(calls) = &message.tool_calls {
                for call in calls {
                    tool_names.insert(&call.id, &call.function.name);
                }
            }
            if let Some(blocks) = &message.content_blocks {
                for block in blocks {
                    if block.get("type").and_then(Value::as_str) == Some("tool_use") {
                        if let (Some(id), Some(name)) = (
                            block.get("id").and_then(Value::as_str),
                            block.get("name").and_then(Value::as_str),
                        ) {
                            tool_names.insert(id, name);
                        }
                    }
                }
            }
        }
        for message in messages {
            let (mut content, attachments) = message_content_tokens(message);
            self.attachment_tokens += attachments;
            if let Some(blocks) = &message.content_blocks {
                for block in blocks {
                    match block.get("type").and_then(Value::as_str) {
                        Some("tool_use") => {
                            let (tokens, _) = block_tokens(block);
                            content = content.saturating_sub(tokens);
                            self.tool_call_tokens += tokens;
                            let name = block.get("name").and_then(Value::as_str).unwrap_or("tool");
                            self.tool_calls.entry(name.to_owned()).or_default().0 += tokens;
                        }
                        Some("tool_result") => {
                            let (tokens, _) = block_tokens(block);
                            content = content.saturating_sub(tokens);
                            self.tool_result_tokens += tokens;
                            let id = block
                                .get("tool_use_id")
                                .and_then(Value::as_str)
                                .unwrap_or("tool");
                            let name = tool_names.get(id).copied().unwrap_or(id);
                            self.tool_calls.entry(name.to_owned()).or_default().1 += tokens;
                        }
                        _ => {}
                    }
                }
            }
            match message.role.as_str() {
                "assistant" => self.assistant_tokens += content,
                "tool" => self.tool_result_tokens += content,
                _ => self.user_tokens += content,
            }
            if let Some(calls) = message.tool_calls.as_ref() {
                // SDK messages can carry both normalized tool_use blocks and
                // OpenAI-compatible tool_calls for the same invocation.
                // The source analyzeContext.ts counts each tool_use once.
                let block_call_ids = message
                    .content_blocks
                    .as_ref()
                    .into_iter()
                    .flatten()
                    .filter(|block| block.get("type").and_then(Value::as_str) == Some("tool_use"))
                    .filter_map(|block| block.get("id").and_then(Value::as_str))
                    .collect::<std::collections::HashSet<_>>();
                for call in calls {
                    if block_call_ids.contains(call.id.as_str()) {
                        continue;
                    }
                    let tokens = estimate(&call.function.name) + estimate(&call.function.arguments);
                    self.tool_call_tokens += tokens;
                    self.tool_calls
                        .entry(call.function.name.clone())
                        .or_default()
                        .0 += tokens;
                }
            }
            if message.role == "tool" {
                let id = message.tool_call_id.as_deref().unwrap_or("tool");
                let name = tool_names.get(id).copied().unwrap_or(id);
                self.tool_calls.entry(name.to_owned()).or_default().1 += content;
            }
        }
    }

    pub fn total_tokens(&self) -> u64 {
        self.system_tokens
            + self.built_in_tokens
            + self.mcp_tokens
            + self.user_tokens
            + self.assistant_tokens
            + self.tool_call_tokens
            + self.tool_result_tokens
            + self.attachment_tokens
    }

    pub fn to_json(
        &self,
        usage: Option<TokenUsage>,
        from_last_request: bool,
        compact_available: bool,
    ) -> Value {
        let window = std::env::var("ATLAS_CONTEXT_WINDOW_TOKENS")
            .ok()
            .and_then(|raw| raw.parse::<u64>().ok())
            .filter(|v| *v > 0);
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
            categories.push(
                json!({"name":"Free space","tokens":limit.saturating_sub(total),
                "color":"secondaryText"}),
            );
        }
        let breakdown = self.tool_calls.iter().map(|(name, (call, result))|
            json!({"name":name,"callTokens":call,"resultTokens":result})
        ).collect::<Vec<_>>();
        let max = window.unwrap_or(0);
        let auto_disabled = ["ATLAS_DISABLE_COMPACT", "ATLAS_DISABLE_AUTO_COMPACT"]
            .iter()
            .any(|key| {
                std::env::var(key).ok().is_some_and(|value| {
                    matches!(
                        value.trim().to_ascii_lowercase().as_str(),
                        "1" | "true" | "yes" | "on"
                    )
                })
            });
        let auto_enabled = compact_available && !auto_disabled;
        let compact_threshold = window
            .map(|limit| {
                if limit > 13_000 {
                    limit - 13_000
                } else {
                    limit - limit / 4
                }
            })
            .unwrap_or(25_000);
        let mut squares = Vec::new();
        if max > 0 {
            let width = if max >= 1_000_000 { 20 } else { 10 };
            let count = width * 10;
            for category in &categories {
                let name = category["name"].as_str().unwrap_or("");
                if name == "Free space" {
                    continue;
                }
                let tokens = category["tokens"].as_u64().unwrap_or(0);
                if tokens == 0 {
                    continue;
                }
                let exact = tokens as f64 / max as f64 * count as f64;
                let squares_for_category = exact.round().max(1.0) as usize;
                let whole = exact.floor() as usize;
                let fraction = exact.fract();
                for index in 0..squares_for_category {
                    if squares.len() >= count {
                        break;
                    }
                    let fullness = if index == whole && fraction > 0.0 {
                        fraction
                    } else {
                        1.0
                    };
                    squares.push(json!({
                        "color":category["color"],
                        "isFilled":true,
                        "categoryName":name,
                        "tokens":tokens,
                        "percentage":(tokens as f64 * 100.0 / max as f64).round(),
                        "squareFullness":fullness
                    }));
                }
            }
            while squares.len() < count {
                squares.push(json!({
                    "color":"promptBorder","isFilled":true,
                    "categoryName":"Free space",
                    "tokens":max.saturating_sub(total),
                    "percentage":(max.saturating_sub(total) as f64 * 100.0 / max as f64).round(),
                    "squareFullness":1.0
                }));
            }
        }
        let grid_rows = if max == 0 {
            Vec::<Vec<Value>>::new()
        } else {
            let width = if max >= 1_000_000 { 20 } else { 10 };
            squares
                .chunks(width)
                .map(|chunk| chunk.to_vec())
                .collect::<Vec<_>>()
        };
        let api_usage = usage.map(|usage| {
            json!({
                "input_tokens":usage.input_tokens,
                "output_tokens":usage.output_tokens,
                "cache_creation_input_tokens":usage.cache_creation_input_tokens,
                "cache_read_input_tokens":usage.cache_read_input_tokens
            })
        });
        json!({
            "categories":categories,
            "totalTokens":total,
            "maxTokens":max,
            "rawMaxTokens":max,
            "percentage":if max > 0 { (total as f64 * 100.0 / max as f64).round() } else { 0.0 },
            "gridRows":grid_rows,
            "model":self.model,
            "memoryFiles":[],
            "mcpTools":self.mcp_tools,
            "systemTools":self.system_tools,
            "agents":[],
            "isAutoCompactEnabled":auto_enabled,
            "autoCompactThreshold":compact_threshold,
            "messageBreakdown":{
                "toolCallTokens":self.tool_call_tokens,
                "toolResultTokens":self.tool_result_tokens,
                "attachmentTokens":self.attachment_tokens,
                "assistantMessageTokens":self.assistant_tokens,
                "userMessageTokens":self.user_tokens,
                "toolCallsByType":breakdown,
                "attachmentsByType":[]
            },
            "apiUsage":api_usage,
            "estimateOnly":true,
            "windowKnown":window.is_some(),
            "fromLastModelRequest":from_last_request
        })
    }

    pub fn render(
        &self,
        usage: Option<TokenUsage>,
        from_last_request: bool,
        compact_available: bool,
    ) -> String {
        let data = self.to_json(usage, from_last_request, compact_available);
        let mut output = format!(
            "## Context Usage\n\nModel: {}\nEstimated tokens: {}\n",
            self.model,
            self.total_tokens()
        );
        if data["windowKnown"] == true {
            output.push_str(&format!(
                "Context window: {} ({}%)\n",
                data["rawMaxTokens"], data["percentage"]
            ));
        } else {
            output.push_str(
                "Context window: unknown (set ATLAS_CONTEXT_WINDOW_TOKENS for percentage)\n",
            );
        }
        if !from_last_request {
            output
                .push_str("Snapshot: reconstructed from current session (no model request yet)\n");
        }
        output.push_str("Token counts are estimates; provider tokenization may differ.\n\n");
        for category in data["categories"].as_array().into_iter().flatten() {
            if category["tokens"].as_u64().unwrap_or(0) > 0 {
                output.push_str(&format!(
                    "{}: {}\n",
                    category["name"].as_str().unwrap_or(""),
                    category["tokens"]
                ));
            }
        }
        output
    }
}
