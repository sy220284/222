//! Original: src/components/MessageModel.tsx.
//! Show the model label only for assistant transcript entries with text content.
use crate::{generated_ports::ui::VNode, portable_ink_core::LineWidthCache};
use serde_json::{json, Value};
use std::collections::BTreeMap;

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
    let Some(model) = message
        .pointer("/message/model")
        .and_then(Value::as_str)
        .filter(|model| !model.is_empty())
    else {
        return VNode::Empty;
    };
    let has_text = message
        .pointer("/message/content")
        .and_then(Value::as_array)
        .is_some_and(|parts| {
            parts
                .iter()
                .any(|part| part.get("type").and_then(Value::as_str) == Some("text"))
        });
    if !has_text {
        return VNode::Empty;
    }
    let width = LineWidthCache::default().line_width(model) + 8;
    VNode::Element {
        tag: "Box".into(),
        attrs: BTreeMap::from([("minWidth".into(), json!(width))]),
        children: vec![VNode::Element {
            tag: "Text".into(),
            attrs: BTreeMap::from([("dimColor".into(), json!(true))]),
            children: vec![VNode::Text(model.into())],
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcript_model_follows_source_visibility_and_width() {
        let message = json!({"type":"assistant","message":{"model":"gpt-6","content":[{"type":"text","text":"hello"}]}});
        assert_eq!(
            render_source_view(&json!({"message":message})),
            VNode::Empty
        );
        assert_eq!(
            render_source_view(
                &json!({"isTranscriptMode":true,"message":{"type":"assistant","message":{"model":"gpt-6","content":[{"type":"tool_use"}]}}})
            ),
            VNode::Empty
        );
        let result = render_source_view(&json!({"isTranscriptMode":true,"message":message}));
        assert_eq!(result.text_content(), "gpt-6");
        assert_eq!(result.unresolved_node_count(), 0);
        let VNode::Element { tag, attrs, .. } = result else {
            panic!("model box")
        };
        assert_eq!(tag, "Box");
        assert_eq!(attrs.get("minWidth"), Some(&json!(13)));
    }
}
