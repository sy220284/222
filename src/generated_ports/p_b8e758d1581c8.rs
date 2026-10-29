//! Native Rust UI structure derived from original: src/components/MessageRow.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"Message".into(),attrs:vec![("message".into(),Value::Null),("lookups".into(),Value::Null),("addMargin".into(),Value::Null),("containerWidth".into(),Value::Null),("tools".into(),Value::Null),("commands".into(),Value::Null),("verbose".into(),Value::Null),("inProgressToolUseIDs".into(),Value::Null),("progressMessagesForMessage".into(),Value::Null),("shouldAnimate".into(),Value::Null),("shouldShowDot".into(),Value::Bool(true)),("isTranscriptMode".into(),Value::Null),("isStatic".into(),Value::Null),("onOpenRateLimitOptions".into(),Value::Null),("isActiveCollapsedGroup".into(),Value::Null),("isUserContinuation".into(),Value::Null),("lastThinkingBlockId".into(),Value::Null),("latestBashOutputUUID".into(),Value::Null)].into_iter().collect(),children:vec![]}
}
