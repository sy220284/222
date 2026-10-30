//! Manual port of `src/components/PromptInput/PromptInputHelpMenu.tsx`.
//! Shortcut bindings, terminal capabilities and app settings are supplied by
//! the host; no default shortcut is used when a customized one is available.
#![allow(dead_code)]
use crate::generated_ports::ui::VNode;
use serde_json::{json,Value};
use std::collections::BTreeMap;

fn node(tag:&str,attrs:BTreeMap<String,Value>,children:Vec<VNode>)->VNode {
    VNode::Element{tag:tag.into(),attrs,children}
}
fn line(content:String,dim:bool)->VNode {
    node("Box",BTreeMap::new(),vec![node("Text",BTreeMap::from([("dimColor".into(),json!(dim))]),vec![VNode::Text(content)])])
}
fn shortcut(props:&Value,key:&str,default:&str)->String {
    props.get("shortcuts").and_then(|x|x.get(key)).and_then(Value::as_str).unwrap_or(default).replace('+'," + ")
}
fn newline_instruction(props:&Value)->String {
    if (props.get("appleTerminal").and_then(Value::as_bool)==Some(true)
        && props.get("platform").and_then(Value::as_str)==Some("macos"))
        || props.get("shiftEnterBindingInstalled").and_then(Value::as_bool)==Some(true) {
        return "shift + ⏎ for newline".into();
    }
    if props.get("hasUsedBackslashReturn").and_then(Value::as_bool)==Some(true) {
        "\\⏎ for newline".into()
    } else { "backslash (\\) + return (⏎) for newline".into() }
}
pub fn render_source_view(props:&Value)->VNode {
    let dim=props.get("dimColor").and_then(Value::as_bool).unwrap_or(false);
    let fixed=props.get("fixedWidth").and_then(Value::as_bool).unwrap_or(false);
    let col1=vec![
        "! for bash mode".to_string(),
        "/ for commands".into(),
        "@ for file paths".into(),
        "& for background".into(),
        "/btw for side question".into(),
    ];
    let mut col2=vec![
        "double tap esc to clear input".into(),
        format!("{} to auto-accept edits",shortcut(props,"chat:cycleMode","shift+tab")),
        format!("{} for verbose output",shortcut(props,"app:toggleTranscript","ctrl+o")),
        format!("{} to toggle tasks",shortcut(props,"app:toggleTodos","ctrl+t")),
    ];
    if props.get("terminalFeatureEnabled").and_then(Value::as_bool)==Some(true)
        && props.get("terminalPanelEnabled").and_then(Value::as_bool)==Some(true) {
        col2.push(format!("{} for terminal",shortcut(props,"app:toggleTerminal","meta+j")));
    }
    col2.push(newline_instruction(props));
    let mut col3=vec![format!("{} to undo",shortcut(props,"chat:undo","ctrl+_"))];
    if props.get("platform").and_then(Value::as_str)!=Some("windows") {
        col3.push("ctrl + z to suspend".into());
    }
    col3.extend([
        format!("{} to paste images",shortcut(props,"chat:imagePaste","ctrl+v")),
        format!("{} to switch model",shortcut(props,"chat:modelPicker","alt+p")),
    ]);
    if props.get("fastModeEnabled").and_then(Value::as_bool)==Some(true)
        && props.get("fastModeAvailable").and_then(Value::as_bool)==Some(true) {
        col3.push(format!("{} to toggle fast mode",shortcut(props,"chat:fastMode","alt+o")));
    }
    col3.extend([
        format!("{} to stash prompt",shortcut(props,"chat:stash","ctrl+s")),
        format!("{} to edit in $EDITOR",shortcut(props,"chat:externalEditor","ctrl+g")),
    ]);
    if props.get("keybindingCustomizationEnabled").and_then(Value::as_bool)==Some(true) {
        col3.push("/keybindings to customize".into());
    }
    let mut first=BTreeMap::from([("flexDirection".into(),json!("column"))]);
    let mut second=first.clone();
    if fixed {
        first.insert("width".into(),json!(24));
        second.insert("width".into(),json!(35));
    }
    let mut outer=BTreeMap::from([("flexDirection".into(),json!("row"))]);
    for key in ["gap","paddingX"] {
        if let Some(value)=props.get(key){outer.insert(key.into(),value.clone());}
    }
    node("Box",outer,vec![
        node("Box",first,col1.into_iter().map(|s|line(s,dim)).collect()),
        node("Box",second,col2.into_iter().map(|s|line(s,dim)).collect()),
        node("Box",BTreeMap::from([("flexDirection".into(),json!("column"))]),
            col3.into_iter().map(|s|line(s,dim)).collect())
    ])
}

#[cfg(test)]
mod parity_tests {
    use super::*;
    #[test]
    fn fixed_width_and_configured_shortcuts_are_displayed(){
        let view=render_source_view(&json!({"fixedWidth":true,"dimColor":true,"gap":2,
            "shortcuts":{"app:toggleTranscript":"ctrl+alt+o","chat:cycleMode":"shift+tab"},
            "platform":"macos","appleTerminal":true}));
        let VNode::Element{children,attrs,..}=&view else {panic!("outer box")};
        assert_eq!(children.len(),3);assert_eq!(attrs["gap"],2);
        let VNode::Element{attrs,..}=&children[0] else {panic!("column")};
        assert_eq!(attrs["width"],24);
        let VNode::Element{attrs,..}=&children[1] else {panic!("column")};
        assert_eq!(attrs["width"],35);
        assert!(view.text_content().contains("ctrl + alt + o for verbose output"));
        assert!(view.text_content().contains("shift + ⏎ for newline"));
    }
    #[test]
    fn feature_gates_and_windows_suspension_are_exact(){
        let p=json!({"platform":"windows","fastModeEnabled":true,"fastModeAvailable":false,
            "terminalFeatureEnabled":true,"terminalPanelEnabled":false});
        let view=render_source_view(&p);let txt=view.text_content();
        assert!(!txt.contains("ctrl + z to suspend"));
        assert!(!txt.contains("toggle fast mode"));
        assert!(!txt.contains("for terminal"));
        assert!(txt.contains("backslash (\\) + return (⏎) for newline"));
    }
}
