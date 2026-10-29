//! Isolated SDK side questions. Reference: source/src/utils/sideQuestion.ts
//! and the fire-and-forget side_question control handler in cli/print.ts.
//! A side question never mutates the main Agent transcript or executes tools.
use crate::{
    config::ProviderProfile,
    protocol::{Message, ModelRequest},
    provider,
};
use anyhow::{bail, Result};
use serde_json::{json, Value};

const SIDE_QUESTION_INSTRUCTIONS: &str = "<system-reminder>This is a side question from the user. Answer directly in a single response. You are a separate, lightweight agent sharing the main conversation context. The main agent is continuing independently. You have no tools: do not read files, run commands, search, or promise to perform actions. There will be no follow-up turn. If the answer is not in the existing conversation context, say so. Do not describe yourself as having interrupted the main agent.</system-reminder>";

pub fn fork_request(
    mut base: ModelRequest,
    question: &str,
) -> Result<ModelRequest> {
    let question = question.trim();
    if question.is_empty() {
        bail!("side_question requires a non-empty question");
    }
    // Keep the exact cached prefix when available. Never expose a tool to
    // the fork, even if the parent had a rich tool directory.
    base.tools.clear();
    base.messages.push(Message::text(
        "user", format!("{SIDE_QUESTION_INSTRUCTIONS}\n\n{question}"),
    ));
    Ok(base)
}

pub async fn answer(profile: ProviderProfile, request: ModelRequest) -> Result<Value> {
    let client = provider::create(&profile)?;
    // One provider completion only; never enter the Agent tool loop.
    let result = client.complete(&request).await?;
    let text = result.message.visible_text().trim().to_owned();
    if !text.is_empty() {
        return Ok(json!({"response":text}));
    }
    if let Some(call) = result.message.tool_calls.as_ref().and_then(|v| v.first()) {
        return Ok(json!({"response":format!(
            "(The model tried to call {} instead of answering directly. Try rephrasing or ask in the main conversation.)",
            call.function.name
        )}));
    }
    Ok(json!({"response":null}))
}
