//! `src/components/TextInput.tsx`: host state -> native input view.
//! The host supplies the state computed by the input editor and animation
//! loop; the source's React hooks are not silently replaced with constants.
#![allow(dead_code)]
use crate::generated_ports::{p_3949a8e5bc4d2,ui::VNode};
use serde_json::{json,Map,Value};
use std::collections::BTreeMap;

pub fn render_source_view(props:&Value)->VNode {
    let state=props.get("inputState").expect("TextInput requires computed inputState from host");
    let mut child:Map<String,Value>=props.as_object().expect("TextInput props object required").clone();
    child.insert("inputState".into(),state.clone());
    let terminal_focus=props.get("terminalFocus").and_then(Value::as_bool).unwrap_or(false);
    child.insert("terminalFocus".into(),json!(terminal_focus));
    child.insert("hidePlaceholderText".into(),json!(props.get("voiceState").and_then(Value::as_str)==Some("recording")));
    VNode::Element{tag:"Box".into(),attrs:BTreeMap::new(),
        children:vec![p_3949a8e5bc4d2::render_source_view(&Value::Object(child))]}
}

#[cfg(test)]
mod parity_tests {
    use super::*;
    #[test]
    fn focused_text_input_routes_input_state_and_voice_recording_placeholder() {
        let normal=render_source_view(&json!({"inputState":{"renderedValue":""},"value":"","placeholder":"speak",
            "terminalFocus":true,"showCursor":true,"focus":true}));
        assert!(normal.text_content().contains("speak"));
        let voice=render_source_view(&json!({"inputState":{"renderedValue":""},"value":"","placeholder":"speak",
            "terminalFocus":true,"showCursor":true,"focus":true,"voiceState":"recording"}));
        assert!(!voice.text_content().contains("speak"));
    }
}
