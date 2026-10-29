//! Rust exports derived from original: src/screens/REPL.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde::{Serialize,Deserialize};
use serde_json::Value;
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
pub enum Screen{
    #[serde(rename="prompt")]
    prompt,
    #[serde(rename="transcript")]
    transcript,
}

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"KeybindingSetup".into(),attrs:vec![].into_iter().collect(),children:vec![VNode::Empty,VNode::Element{tag:"AnimatedTerminalTitle".into(),attrs:vec![("isAnimating".into(),Value::Null),("title".into(),Value::Null),("disabled".into(),Value::Null),("noPrefix".into(),Value::Null)].into_iter().collect(),children:vec![]},VNode::Empty,VNode::Element{tag:"GlobalKeybindingHandlers".into(),attrs:vec![].into_iter().collect(),children:vec![]},VNode::Empty,VNode::Value(if crate::generated_ports::runtime::truthy(&Value::Null){Value::Null}else{Value::Null}),VNode::Empty,VNode::Element{tag:"CommandKeybindingHandlers".into(),attrs:vec![("onSubmit".into(),Value::Null),("isActive".into(),Value::Bool(!crate::generated_ports::runtime::truthy(&crate::generated_ports::runtime::property(&Value::Null,"isLocalJSXCommand"))))].into_iter().collect(),children:vec![]},VNode::Empty,VNode::Empty,VNode::Empty,VNode::Element{tag:"ScrollKeybindingHandler".into(),attrs:vec![("scrollRef".into(),Value::Null),("isActive".into(),Value::Null),("onScroll".into(),if crate::generated_ports::runtime::truthy(&Value::Null){Value::Null}else{Value::Null})].into_iter().collect(),children:vec![]},VNode::Empty,VNode::Value(if crate::generated_ports::runtime::truthy(&Value::Null){Value::Null}else{Value::Null}),VNode::Empty,VNode::Element{tag:"CancelRequestHandler".into(),attrs:vec![].into_iter().collect(),children:vec![]},VNode::Empty,VNode::Element{tag:"MCPConnectionManager".into(),attrs:vec![("key".into(),Value::Null),("dynamicMcpConfig".into(),Value::Null),("isStrictMcpConfig".into(),Value::Null)].into_iter().collect(),children:vec![VNode::Empty,VNode::Element{tag:"FullscreenLayout".into(),attrs:vec![("scrollRef".into(),Value::Null),("overlay".into(),Value::Null),("bottomFloat".into(),if crate::generated_ports::runtime::truthy(&Value::Null){Value::Null}else{Value::Null}),("modal".into(),Value::Null),("modalScrollRef".into(),Value::Null),("dividerYRef".into(),Value::Null),("hidePill".into(),Value::Bool(!crate::generated_ports::runtime::truthy(&Value::Bool(!crate::generated_ports::runtime::truthy(&Value::Null))))),("hideSticky".into(),Value::Bool(!crate::generated_ports::runtime::truthy(&Value::Bool(!crate::generated_ports::runtime::truthy(&Value::Null))))),("newMessageCount".into(),Value::Null),("onPillClick".into(),Value::Null),("scrollable".into(),Value::Null),("bottom".into(),Value::Null)].into_iter().collect(),children:vec![]},VNode::Empty]},VNode::Empty]}
}
