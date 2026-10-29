//! Native Rust UI structure derived from original: src/components/Message.tsx
#![allow(dead_code, non_snake_case, non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props: &Value) -> VNode {
    VNode::Element {
        tag: "AssistantMessageBlock".into(),
        attrs: vec![
            ("key".into(), Value::Null),
            ("param".into(), Value::Null),
            ("addMargin".into(), Value::Null),
            ("tools".into(), Value::Null),
            ("commands".into(), Value::Null),
            ("verbose".into(), Value::Null),
            ("inProgressToolUseIDs".into(), Value::Null),
            ("progressMessagesForMessage".into(), Value::Null),
            ("shouldAnimate".into(), Value::Null),
            ("shouldShowDot".into(), Value::Null),
            ("width".into(), Value::Null),
            (
                "inProgressToolCallCount".into(),
                crate::generated_ports::runtime::property(&Value::Null, "size"),
            ),
            ("isTranscriptMode".into(), Value::Null),
            ("lookups".into(), Value::Null),
            ("onOpenRateLimitOptions".into(), Value::Null),
            ("thinkingBlockId".into(), Value::Null),
            ("lastThinkingBlockId".into(), Value::Null),
            (
                "advisorModel".into(),
                crate::generated_ports::runtime::property(&Value::Null, "advisorModel"),
            ),
        ]
        .into_iter()
        .collect(),
        children: vec![],
    }
}
