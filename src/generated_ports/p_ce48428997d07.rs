//! Original: src/components/MessageTimestamp.tsx.
//! Render transcript assistant text timestamps using the host's local time zone.
use crate::{generated_ports::ui::VNode, portable_ink_core::LineWidthCache};
use chrono::{DateTime, Local, TimeZone};
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn display_time(timestamp: &Value) -> Option<String> {
    let instant = if let Some(ms) = timestamp.as_i64() {
        Local.timestamp_millis_opt(ms).single()?
    } else {
        let value = timestamp.as_str()?;
        if let Ok(dt) = DateTime::parse_from_rfc3339(value) {
            dt.with_timezone(&Local)
        } else if let Ok(dt) = DateTime::parse_from_rfc2822(value) {
            dt.with_timezone(&Local)
        } else {
            return None;
        }
    };
    Some(instant.format("%I:%M %p").to_string())
}

pub fn render_source_view(props: &Value) -> VNode {
    if props.get("isTranscriptMode").and_then(Value::as_bool) != Some(true) {
        return VNode::Empty;
    }
    let Some(message) = props.get("message") else {
        return VNode::Empty;
    };
    if message.get("type").and_then(Value::as_str) != Some("assistant") {
        return VNode::Empty;
    }
    let text_content = message
        .pointer("/message/content")
        .and_then(Value::as_array)
        .is_some_and(|parts| {
            parts
                .iter()
                .any(|part| part.get("type").and_then(Value::as_str) == Some("text"))
        });
    if !text_content {
        return VNode::Empty;
    }
    let Some(when) = message.get("timestamp").filter(|value| !value.is_null()) else {
        return VNode::Empty;
    };
    let Some(label) = display_time(when) else {
        return VNode::Empty;
    };
    let width = LineWidthCache::default().line_width(&label);
    VNode::Element {
        tag: "Box".into(),
        attrs: BTreeMap::from([("minWidth".into(), json!(width))]),
        children: vec![VNode::Element {
            tag: "Text".into(),
            attrs: BTreeMap::from([("dimColor".into(), json!(true))]),
            children: vec![VNode::Text(label)],
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timestamp_requires_transcript_assistant_text() {
        let msg = json!({"type":"assistant","timestamp":"2026-10-29T14:00:00Z","message":{"content":[{"type":"text","text":"hello"}]}});
        assert_eq!(render_source_view(&json!({"message":msg})), VNode::Empty);
        assert_eq!(
            render_source_view(
                &json!({"isTranscriptMode":true,"message":{"type":"assistant","timestamp":"2026-10-29T14:00:00Z","message":{"content":[{"type":"thinking"}]}}})
            ),
            VNode::Empty
        );
        let out = render_source_view(&json!({"isTranscriptMode":true,"message":msg}));
        let expected = DateTime::parse_from_rfc3339("2026-10-29T14:00:00Z")
            .unwrap()
            .with_timezone(&Local)
            .format("%I:%M %p")
            .to_string();
        assert_eq!(out.text_content(), expected);
        assert_eq!(out.unresolved_node_count(), 0);
        let VNode::Element { attrs, .. } = out else {
            panic!("expected box")
        };
        assert_eq!(attrs.get("minWidth"), Some(&json!(expected.len())));
    }
    #[test]
    fn accepts_js_epoch_milliseconds_and_rejects_invalid_date() {
        assert!(display_time(&json!(0)).is_some());
        assert_eq!(display_time(&json!("bad date")), None);
    }
}
