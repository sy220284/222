//! `src/components/VimTextInput.tsx`: Vim editor state -> native input view.
#![allow(dead_code)]
use crate::generated_ports::{p_3949a8e5bc4d2,ui::VNode};
use serde_json::{json,Map,Value};
use std::collections::BTreeMap;

pub fn render_source_view(props:&Value)->VNode {
    let state=props.get("vimInputState").or_else(||props.get("inputState"))
        .expect("VimTextInput requires vimInputState from host");
    let mut child:Map<String,Value>=props.as_object().expect("VimTextInput props object required").clone();
    child.insert("inputState".into(),state.clone());
    child.insert("terminalFocus".into(),json!(props.get("terminalFocus").and_then(Value::as_bool).unwrap_or(false)));
    VNode::Element{tag:"Box".into(),attrs:BTreeMap::from([("flexDirection".into(),json!("column"))]),
        children:vec![p_3949a8e5bc4d2::render_source_view(&Value::Object(child))]}
}

#[cfg(test)]
mod parity_tests {
    use super::*;
    #[test]
    fn vim_input_displays_the_live_editor_snapshot() {
        let node=render_source_view(&json!({"value":"hello","vimInputState":{"renderedValue":"hello"},"terminalFocus":true}));
        assert_eq!(node.text_content(),"hello");
    }
}
