//! Source parity for utils/processUserInput/processTextPrompt.ts.
//!
//! The provider-neutral Rust boundary preserves structured SDK images and
//! other content blocks in the exact order supplied. Provider telemetry
//! remains outside the user-visible prompt pipeline.
use crate::protocol::Message;
use serde_json::Value;

/// These blocks are model inputs, not a textual prompt. In particular,
/// an image-only request remains a valid turn without synthetic text.
pub fn has_content_blocks(blocks: Option<&[Value]>) -> bool {
    blocks.is_some_and(|blocks| !blocks.is_empty())
}

pub struct ProcessedPrompt {
    pub message: Message,
    pub first_text: String,
    pub last_text: String,
    pub should_query: bool,
}

/// Convert CLI text or SDK blocks into the same durable user-message
/// representation used by the agent and every provider backend.
pub fn process_text_prompt(text: &str, blocks: Option<Vec<Value>>) -> ProcessedPrompt {
    let first_text = blocks
        .as_ref()
        .and_then(|parts| {
            parts.iter().find_map(|part| {
                (part.get("type").and_then(Value::as_str) == Some("text"))
                    .then(|| part.get("text").and_then(Value::as_str))
                    .flatten()
            })
        })
        .unwrap_or(text)
        .to_owned();
    let last_text = blocks
        .as_ref()
        .and_then(|parts| {
            parts.iter().rev().find_map(|part| {
                (part.get("type").and_then(Value::as_str) == Some("text"))
                    .then(|| part.get("text").and_then(Value::as_str))
                    .flatten()
            })
        })
        .unwrap_or(text)
        .to_owned();
    let mut message = Message::text("user", text);
    message.content_blocks = blocks;
    ProcessedPrompt {
        message,
        first_text,
        last_text,
        should_query: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn text_only_keeps_original_whitespace_and_queries() {
        let result = process_text_prompt(" hello\nworld ", None);
        assert_eq!(result.message.content.as_deref(), Some(" hello\nworld "));
        assert!(result.message.content_blocks.is_none());
        assert_eq!(result.first_text, " hello\nworld ");
        assert_eq!(result.last_text, " hello\nworld ");
        assert!(result.should_query);
        // The source processTextPrompt always returns shouldQuery=true;
        // empty-input rejection belongs to the outer CLI input policy.
        assert!(process_text_prompt("  \n ", None).should_query);
        assert!(process_text_prompt("", Some(vec![])).should_query);
    }

    #[test]
    fn image_only_sdk_prompt_does_not_require_fake_text() {
        let block = json!({"type":"image","source":{"type":"base64","media_type":"image/png","data":"AA==" }});
        let result = process_text_prompt("", Some(vec![block.clone()]));
        assert_eq!(result.message.role, "user");
        assert_eq!(result.message.content.as_deref(), Some(""));
        assert_eq!(result.message.content_blocks, Some(vec![block]));
        assert_eq!(result.first_text, "");
        assert!(result.should_query);
    }

    #[test]
    fn mixed_sdk_content_keeps_order_and_first_last_text_independent() {
        let parts = vec![
            json!({"type":"image","source":"local"}),
            json!({"type":"text","text":"first"}),
            json!({"type":"image","source":"later"}),
            json!({"type":"text","text":"last"}),
        ];
        let result = process_text_prompt("first", Some(parts.clone()));
        assert_eq!(result.message.content_blocks, Some(parts));
        assert_eq!(result.first_text, "first");
        assert_eq!(result.last_text, "last");
    }
}
