//! Native Rust UI structure derived from original: src/commands/mcp/mcp.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"MCPToggle".into(),attrs:vec![("action".into(),crate::generated_ports::runtime::index(&Value::Null,&serde_json::json!(0))),("target".into(),if crate::generated_ports::runtime::truthy(&Value::Null){Value::Null}else{Value::String("all".into())}),("onComplete".into(),Value::Null)].into_iter().collect(),children:vec![]}
}
