//! Headless queue prompt batching from src/cli/print.ts.
//! Joining is independent from delivery so each request retains its own identity.
use crate::message_queue::{Command, Mode};
use serde_json::{json, Value};

pub fn can_batch_with(head: &Command, next: Option<&Command>) -> bool {
    head.mode == Mode::Prompt
        && next.is_some_and(|next| {
            next.mode == Mode::Prompt
                && next.workload == head.workload
                && next.is_meta == head.is_meta
        })
}

/// Original joinPromptValues preserves all structured content blocks. Plain
/// strings join with newlines; a mixed batch becomes a flattened block array.
pub fn join_prompt_values(values: &[Value]) -> Value {
    if values.len() == 1 {
        return values[0].clone();
    }
    if values.iter().all(Value::is_string) {
        return Value::String(
            values
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    let mut blocks = Vec::new();
    for value in values {
        match value {
            Value::String(text) => blocks.push(json!({"type":"text","text":text})),
            Value::Array(items) => blocks.extend(items.iter().cloned()),
            _ => {}
        }
    }
    Value::Array(blocks)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::message_queue::Priority;
    use serde_json::json;
    fn command(value: &str, workload: Option<&str>, meta: bool) -> Command {
        Command {
            value: json!(value),
            mode: Mode::Prompt,
            priority: Some(Priority::Next),
            uuid: None,
            orphaned_permission: None,
            pasted_contents: None,
            pre_expansion_value: None,
            skip_slash_commands: false,
            bridge_origin: false,
            is_meta: meta,
            origin: None,
            workload: workload.map(str::to_owned),
            agent_id: None,
        }
    }
    #[test]
    fn only_matching_workload_and_meta_batch_together() {
        let head = command("a", None, false);
        assert!(can_batch_with(&head, Some(&command("b", None, false))));
        assert!(!can_batch_with(
            &head,
            Some(&command("b", Some("cron"), false))
        ));
        assert!(!can_batch_with(&head, Some(&command("b", None, true))));
        assert!(!can_batch_with(&head, None));
        let mut note = command("meta", None, false);
        note.mode = Mode::TaskNotification;
        assert!(!can_batch_with(&head, Some(&note)));
    }
    #[test]
    fn strings_join_with_newlines_and_mixed_blocks_preserve_order() {
        assert_eq!(join_prompt_values(&[json!("a"), json!("b")]), json!("a\nb"));
        assert_eq!(join_prompt_values(&[json!("single")]), json!("single"));
        assert_eq!(
            join_prompt_values(&[
                json!("a"),
                json!([{"type":"text","text":"b"},
                {"type":"image","source":{"type":"base64","data":"AA","media_type":"image/png"}}])
            ]),
            json!([
                {"type":"text","text":"a"},{"type":"text","text":"b"},
                {"type":"image","source":{"type":"base64","data":"AA","media_type":"image/png"}}
            ])
        );
    }
}
