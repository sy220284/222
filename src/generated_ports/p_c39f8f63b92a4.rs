//! `src/components/PromptInput/PromptInputFooter.tsx` view and overlay logic.
//! All selector/hook values are explicit input state; terminal and coordinator
//! state must be supplied by the host rather than replaced with a constant.
#![allow(dead_code)]
use crate::generated_ports::ui::VNode;
use serde_json::{json,Value};
use std::collections::BTreeMap;

fn bool_prop(props:&Value,key:&str)->bool {props.get(key).and_then(Value::as_bool).unwrap_or(false)}
fn vnode(tag:&str,attrs:BTreeMap<String,Value>,children:Vec<VNode>)->VNode {
    VNode::Element{tag:tag.into(),attrs,children}
}
fn attrs(props:&Value,names:&[&str])->BTreeMap<String,Value> {
    names.iter().filter_map(|key|props.get(key).map(|v|((*key).into(),v.clone()))).collect()
}
fn terminal_fullscreen(props:&Value)->bool {bool_prop(props,"isFullscreen")}
fn columns(props:&Value)->usize {props.get("terminalColumns").and_then(Value::as_u64).unwrap_or(80) as usize}
fn rows(props:&Value)->usize {props.get("terminalRows").and_then(Value::as_u64).unwrap_or(24) as usize}
fn status_line_enabled(props:&Value)->bool {
    props.get("settings").and_then(Value::as_object)
        .is_some_and(|settings|settings.contains_key("statusLine"))
}

/// The source portals suggestions into FullscreenLayout rather than drawing
/// a duplicate list in the small footer slot.
pub fn prompt_overlay_data(props:&Value)->Value {
    let suggestions=props.get("suggestions").and_then(Value::as_array).expect("suggestions are required");
    if !terminal_fullscreen(props) || suggestions.is_empty() {return Value::Null;}
    let mut map=serde_json::Map::new();
    map.insert("suggestions".into(),json!(suggestions));
    if let Some(selected)=props.get("selectedSuggestion") {map.insert("selectedSuggestion".into(),selected.clone());}
    if let Some(width)=props.get("maxColumnWidth") {map.insert("maxColumnWidth".into(),width.clone());}
    Value::Object(map)
}

fn bridge_status(props:&Value)->Option<VNode> {
    // Mirrors the source's compile-time BRIDGE_MODE gate, the runtime config
    // gate, and the per-session 'enabled' state.
    if !bool_prop(props,"featureBridgeMode") || !bool_prop(props,"bridgeEnabled")
        || !bool_prop(props,"replBridgeEnabled") {return None;}
    let (label,color)=if bool_prop(props,"replBridgeReconnecting") {
        ("Remote Control reconnecting","warning")
    } else if bool_prop(props,"replBridgeSessionActive") || bool_prop(props,"replBridgeConnected") {
        ("Remote Control active","success")
    } else { ("Remote Control connecting…","warning") };
    let explicit=bool_prop(props,"replBridgeExplicit");
    if !explicit && label!="Remote Control reconnecting" {return None;}
    let selected=bool_prop(props,"bridgeSelected");
    let mut children=vec![VNode::Text(label.into())];
    if selected {children.push(vnode("Text",BTreeMap::from([("dimColor".into(),json!(true))]),
                                 vec![VNode::Text(" · Enter to view".into())]));}
    Some(vnode("Text",BTreeMap::from([
        ("color".into(),json!(if selected {"background"} else {color})),
        ("inverse".into(),json!(selected)),("wrap".into(),json!("truncate"))
    ]),children))
}

pub fn render_source_view(props:&Value)->VNode {
    let suggestions=props.get("suggestions").and_then(Value::as_array).expect("suggestions are required");
    let fullscreen=terminal_fullscreen(props);
    if !suggestions.is_empty() && !fullscreen {
        return vnode("Box",BTreeMap::from([("paddingX".into(),json!(2)),("paddingY".into(),json!(0))]),
            vec![vnode("PromptInputFooterSuggestions",attrs(props,&[
                "suggestions","selectedSuggestion","maxColumnWidth"]),vec![])]);
    }
    if bool_prop(props,"helpOpen") {
        return vnode("PromptInputHelpMenu",BTreeMap::from([
            ("dimColor".into(),json!(true)),("fixedWidth".into(),json!(true)),("paddingX".into(),json!(2))]),vec![]);
    }
    let narrow=columns(props)<80;
    let short=fullscreen && rows(props)<24;
    let mode=props.get("mode").and_then(Value::as_str).unwrap_or("");
    let is_searching=bool_prop(props,"isSearching");
    let is_pasting=bool_prop(props,"isPasting");
    let status_line=status_line_enabled(props);
    let suppress_hint=bool_prop(props,"suppressHint") || status_line || is_searching;
    let task_count=props.get("coordinatorTaskCount").and_then(Value::as_i64).unwrap_or(0);
    let selected_index=props.get("coordinatorTaskIndex").and_then(Value::as_i64).unwrap_or(-1);
    let pill_selected=bool_prop(props,"tasksSelected") && (task_count==0 || selected_index<0);

    let mut left_children=vec![];
    if mode=="prompt" && !short && !props.get("exitMessage").is_some_and(|v|bool_prop(v,"show"))
        && !is_pasting && status_line {
        left_children.push(vnode("StatusLine",attrs(props,&["messages","lastAssistantMessageId","vimMode"]),vec![]));
    }
    let mut left_attrs=attrs(props,&["exitMessage","vimMode","mode","toolPermissionContext",
        "isLoading","teamsSelected","teammateFooterIndex","tmuxSelected","isPasting",
        "isSearching","historyQuery","historyFailedMatch"]);
    left_attrs.insert("suppressHint".into(),json!(suppress_hint));
    left_attrs.insert("tasksSelected".into(),json!(pill_selected));
    left_children.push(vnode("PromptInputFooterLeftSide",left_attrs,vec![]));
    let left=vnode("Box",BTreeMap::from([
        ("flexDirection".into(),json!("column")),("flexShrink".into(),json!(if narrow {0} else {1}))]),left_children);

    let mut right_children=vec![];
    if !fullscreen {
        let mut notification_attrs=attrs(props,&["apiKeyStatus","autoUpdaterResult","debug",
            "isAutoUpdating","verbose","messages","ideSelection","mcpClients","isInputWrapped"]);
        notification_attrs.insert("isNarrow".into(),json!(narrow));
        right_children.push(vnode("Notifications",notification_attrs,vec![]));
    }
    if let Some(bridge)=bridge_status(props) { right_children.push(bridge); }
    let right=vnode("Box",BTreeMap::from([("flexShrink".into(),json!(1)),("gap".into(),json!(1))]),right_children);
    VNode::Group(vec![vnode("Box",BTreeMap::from([
        ("flexDirection".into(),json!(if narrow {"column"} else {"row"})),
        ("justifyContent".into(),json!(if narrow {"flex-start"} else {"space-between"})),
        ("paddingX".into(),json!(2)),("gap".into(),json!(if narrow {0} else {1}))]),vec![left,right])])
}

#[cfg(test)]
mod parity_tests {
    use super::*;
    fn props()->Value {json!({"suggestions":[],"mode":"prompt","terminalColumns":100,"terminalRows":40})}
    #[test]
    fn suggestions_are_inline_only_outside_fullscreen() {
        let mut p=props();p["suggestions"]=json!([{"label":"a"}]);
        let view=render_source_view(&p);
        let VNode::Element{tag,children,..}=view else {panic!("Box")};
        assert_eq!(tag,"Box");
        let VNode::Element{tag,..}=&children[0] else {panic!("suggestions")};
        assert_eq!(tag,"PromptInputFooterSuggestions");
        p["isFullscreen"]=json!(true);
        assert_eq!(prompt_overlay_data(&p)["suggestions"].as_array().unwrap().len(),1);
        assert_ne!(render_source_view(&p),vnode("PromptInputFooterSuggestions",BTreeMap::new(),vec![]));
    }
    #[test]
    fn narrow_short_help_and_custom_status_line_conditions() {
        let mut p=props();p["terminalColumns"]=json!(40);p["isFullscreen"]=json!(true);
        p["terminalRows"]=json!(20);p["settings"]=json!({"statusLine":{}});
        let rendered=render_source_view(&p);
        let VNode::Group(children)=rendered else {panic!("group")};
        let VNode::Element{attrs,children,..}=&children[0] else {panic!("outer")};
        assert_eq!(attrs["flexDirection"],"column");
        let VNode::Element{children,..}=&children[0] else {panic!("left")};
        assert_eq!(children.len(),1); // Short fullscreen omits optional StatusLine.
        p["helpOpen"]=json!(true);
        let VNode::Element{tag,..}=render_source_view(&p) else {panic!("help")};
        assert_eq!(tag,"PromptInputHelpMenu");
    }
    #[test]
    fn bridge_status_selection_and_implicit_reconnect_only() {
        let mut p=props();p["featureBridgeMode"]=json!(true);p["bridgeEnabled"]=json!(true);
        p["replBridgeEnabled"]=json!(true);p["replBridgeExplicit"]=json!(false);
        assert!(bridge_status(&p).is_none());
        p["replBridgeReconnecting"]=json!(true);
        assert_eq!(bridge_status(&p).unwrap().text_content(),"Remote Control reconnecting");
        p["bridgeSelected"]=json!(true);
        assert!(bridge_status(&p).unwrap().text_content().contains("Enter to view"));
    }
}
