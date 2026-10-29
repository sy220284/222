//! Native Rust UI structure derived from original: src/components/CustomSelect/SelectMulti.tsx
#![allow(dead_code,non_snake_case,non_camel_case_types)]
use serde_json::Value;

use crate::generated_ports::ui::VNode;
pub fn render_source_view(props:&Value)->VNode{
VNode::Element{tag:"Box".into(),attrs:vec![("key".into(),Value::Null),("gap".into(),serde_json::json!(1))].into_iter().collect(),children:vec![VNode::Element{tag:"SelectInputOption".into(),attrs:vec![("option".into(),Value::Null),("isFocused".into(),Value::Null),("isSelected".into(),Value::Bool(false)),("shouldShowDownArrow".into(),Value::Null),("shouldShowUpArrow".into(),Value::Null),("maxIndexWidth".into(),Value::Null),("index".into(),Value::Null),("inputValue".into(),Value::Null),("onInputChange".into(),Value::Null),("onSubmit".into(),Value::Null),("onExit".into(),Value::Null),("layout".into(),Value::String("compact".into())),("onOpenEditor".into(),Value::Null),("onImagePaste".into(),Value::Null),("pastedContents".into(),Value::Null),("onRemoveImage".into(),Value::Null)].into_iter().collect(),children:vec![VNode::Element{tag:"Text".into(),attrs:vec![("color".into(),if crate::generated_ports::runtime::truthy(&Value::Null){Value::String("success".into())}else{Value::Null})].into_iter().collect(),children:vec![VNode::Text("[".into()),VNode::Value(if crate::generated_ports::runtime::truthy(&Value::Null){crate::generated_ports::runtime::property(&Value::Null,"tick")}else{Value::String(" ".into())}),VNode::Text("]".into()),VNode::Value(Value::String(" ".into()))]}]}]}
}
