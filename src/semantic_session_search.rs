//! Model-ranked session search, based on 2.1.88 utils/agenticSessionSearch.ts.
//! Literal prefilter first; recent nonmatches fill a maximum of 100 candidates.
use crate::{
    protocol::{Message, ModelRequest, TokenUsage},
    provider::ModelProvider,
    session::Session,
    session_control::{self, SessionItem},
};
use anyhow::Result;
use serde_json::Value;
use std::{collections::HashSet, path::Path};

const MAX_SESSIONS: usize = 100;
const MAX_TRANSCRIPT: usize = 2_000;
const SYSTEM: &str = r#"Find relevant conversation sessions by title, tag, summary, first message and transcript.
Rank exact tag matches first, followed by partial tags, title, summary, transcript and
semantic similarity. Be inclusive, but do not invent sessions. Return only JSON
with an array of zero-based candidate indices: {"relevant_indices":[2,5,0]}."#;

fn excerpt(messages: &[Message]) -> String {
    let selected: Vec<_> = if messages.len() <= 100 {
        messages.iter().collect()
    } else {
        messages
            .iter()
            .take(50)
            .chain(messages.iter().skip(messages.len() - 50))
            .collect()
    };
    let text = selected
        .into_iter()
        .filter(|message| matches!(message.role.as_str(), "user" | "assistant"))
        .map(|message| {
            let mut parts = Vec::new();
            if let Some(text) = message.content.as_deref() {
                parts.push(text);
            }
            if let Some(blocks) = &message.content_blocks {
                for block in blocks {
                    if let Some(text) = block
                        .as_str()
                        .or_else(|| block.get("text").and_then(Value::as_str))
                    {
                        parts.push(text);
                    }
                }
            }
            parts.join(" ")
        })
        .collect::<Vec<_>>()
        .join(" ");
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    // JS String.slice measures UTF-16 code units, not Unicode scalar values.
    // Preserve the original 2,000-unit limit without splitting a surrogate pair.
    let mut units = 0usize;
    for (offset, ch) in normalized.char_indices() {
        if units + ch.len_utf16() > MAX_TRANSCRIPT {
            return format!("{}…", &normalized[..offset]);
        }
        units += ch.len_utf16();
    }
    normalized
}

fn prompt(query: &str, candidates: &[(SessionItem, Option<Session>)]) -> String {
    let descriptions = candidates
        .iter()
        .enumerate()
        .map(|(index, (item, loaded))| {
            let mut fields = vec![format!("{index}: {}", item.title)];
            if let Some(tag) = item.tag.as_deref() {
                fields.push(format!("[tag: {tag}]"));
            }
            if let Some(session) = loaded {
                // The original search includes the custom title separately
                // when a session's display name comes from another source.
                // Omitting it makes renamed conversations hard to rediscover.
                if let Some(custom_title) = session
                    .custom_title
                    .as_deref()
                    .filter(|title| *title != item.title)
                {
                    fields.push(format!("[custom title: {custom_title}]"));
                }
                if let Some(summary) = session.summary.as_deref() {
                    // The original sends the full summary to the ranker.
                    fields.push(format!("- Summary: {summary}"));
                }
                if let Some(first) = session.messages.iter().find(|m| m.role == "user") {
                    let text = excerpt(std::slice::from_ref(first));
                    if !text.is_empty() {
                        fields.push(format!(
                            "- First message: {}",
                            text.chars().take(300).collect::<String>()
                        ));
                    }
                }
                let text = excerpt(&session.messages);
                if !text.is_empty() {
                    fields.push(format!("- Transcript: {text}"));
                }
            }
            fields.join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("Sessions:\n{descriptions}\n\nSearch query: \"{query}\"\n\nFind relevant sessions.")
}

fn parse_indices(text: &str, limit: usize) -> Vec<usize> {
    let (Some(start), Some(end)) = (text.find('{'), text.rfind('}')) else {
        return Vec::new();
    };
    if end < start {
        return Vec::new();
    }
    let Ok(value) = serde_json::from_str::<Value>(&text[start..=end]) else {
        return Vec::new();
    };
    let Some(indices) = value.get("relevant_indices").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut seen = HashSet::new();
    indices
        .iter()
        .filter_map(Value::as_u64)
        .filter_map(|index| usize::try_from(index).ok())
        .filter(|index| *index < limit && seen.insert(*index))
        .collect()
}

/// Explicit opt-in: ordinary local session listing never sends transcripts
/// to a model. The provider is user-selected and independent of any vendor.
pub async fn search(
    root: &Path,
    query: &str,
    provider: &dyn ModelProvider,
    model: &str,
) -> Result<(Vec<SessionItem>, Option<TokenUsage>)> {
    if query.trim().is_empty() {
        return Ok((Vec::new(), None));
    }
    let recent = session_control::list(root)?;
    if recent.is_empty() {
        return Ok((Vec::new(), None));
    }
    let matching = session_control::search(root, query)?;
    let matching_ids: HashSet<_> = matching.iter().map(|item| item.id).collect();
    let candidates = matching
        .into_iter()
        .chain(
            recent
                .into_iter()
                .filter(|item| !matching_ids.contains(&item.id)),
        )
        .take(MAX_SESSIONS)
        .map(|item| {
            let loaded = Session::load(root, &item.id.to_string()).ok();
            (item, loaded)
        })
        .collect::<Vec<_>>();
    // The original agenticSessionSearch catches provider failures and returns
    // an empty result rather than aborting the session/CLI command.
    let result = match provider
        .complete(&ModelRequest {
            model: model.to_owned(),
            system: SYSTEM.into(),
            messages: vec![Message::text("user", prompt(query, &candidates))],
            tools: Vec::new(),
        })
        .await
    {
        Ok(response) => response,
        Err(error) => {
            eprintln!("Semantic session search failed: {error:#}");
            return Ok((Vec::new(), None));
        }
    };
    let ranked = result
        .message
        .content
        .as_deref()
        .map(|text| parse_indices(text, candidates.len()))
        .unwrap_or_default()
        .into_iter()
        .map(|index| candidates[index].0.clone())
        .collect();
    Ok((ranked, result.usage))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::ModelResponse;
    use anyhow::Result;
    use async_trait::async_trait;
    use std::sync::Mutex;

    struct Ranker {
        answer: String,
        calls: Mutex<Vec<String>>,
    }
    #[async_trait]
    impl ModelProvider for Ranker {
        async fn complete(&self, request: &ModelRequest) -> Result<ModelResponse> {
            self.calls
                .lock()
                .unwrap()
                .push(request.messages[0].content.clone().unwrap_or_default());
            Ok(ModelResponse {
                message: Message::text("assistant", &self.answer),
                finish_reason: Some("stop".into()),
                usage: Some(TokenUsage {
                    input_tokens: 2,
                    output_tokens: 3,
                    total_tokens: 5,
                    ..TokenUsage::default()
                }),
            })
        }
    }
    #[test]
    fn invalid_and_duplicate_indices_are_ignored() {
        assert_eq!(
            parse_indices("Here: {\"relevant_indices\":[2,2,0,-1,90,\"1\"]}", 3),
            vec![2, 0]
        );
        assert!(parse_indices("nonsense", 3).is_empty());
    }
    #[test]
    fn transcript_is_first_and_last_50_and_at_most_2000_chars() {
        let messages: Vec<_> = (0..110)
            .map(|i| Message::text("user", format!("entry-{i}")))
            .collect();
        let result = excerpt(&messages);
        assert!(result.contains("entry-0"));
        assert!(result.contains("entry-109"));
        assert!(!result.contains("entry-55"));
        let capped = excerpt(&[Message::text("user", "x".repeat(3_000))]);
        assert_eq!(capped.chars().count(), 2_001);
        assert!(capped.ends_with('…'));
    }
    #[tokio::test]
    async fn real_sessions_are_ranked_by_model_and_usage_is_returned() {
        let root = tempfile::tempdir().unwrap();
        let mut relevant = Session::fresh();
        relevant.custom_title = Some("Release notes".into());
        relevant.tag = Some("release".into());
        relevant.summary = Some("Compiled changelog".into());
        relevant
            .messages
            .push(Message::text("user", "Publish notes"));
        relevant.save(root.path()).unwrap();
        let mut other = Session::fresh();
        other.custom_title = Some("Review auth".into());
        other.messages.push(Message::text("user", "Improve login"));
        other.save(root.path()).unwrap();
        let ranker = Ranker {
            answer: "{\"relevant_indices\":[1,0,1,500]}".into(),
            calls: Mutex::new(Vec::new()),
        };
        let (ranked, usage) = search(root.path(), "release", &ranker, "local")
            .await
            .unwrap();
        assert_eq!(
            ranked.iter().map(|item| item.id).collect::<Vec<_>>(),
            vec![other.id, relevant.id]
        );
        assert_eq!(usage.unwrap().total_tokens, 5);
        let prompt = ranker.calls.lock().unwrap()[0].clone();
        assert!(prompt.contains("[tag: release]"));
        assert!(prompt.contains("Compiled changelog"));
        assert!(prompt.contains("Improve login"));
    }
    #[tokio::test]
    async fn blank_query_never_invokes_model() {
        let root = tempfile::tempdir().unwrap();
        let ranker = Ranker {
            answer: String::new(),
            calls: Mutex::new(Vec::new()),
        };
        let (results, usage) = search(root.path(), " ", &ranker, "local").await.unwrap();
        assert!(results.is_empty());
        assert!(usage.is_none());
        assert!(ranker.calls.lock().unwrap().is_empty());
    }
}
